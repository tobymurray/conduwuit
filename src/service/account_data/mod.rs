use std::{collections::HashMap, sync::Arc};

use conduit::{
	implement,
	utils::{stream::TryIgnore, ReadyExt},
	Err, Error, Result,
};
use database::{Deserialized, Map};
use futures::{StreamExt, TryFutureExt};
use ruma::{
	events::{AnyGlobalAccountDataEvent, AnyRawAccountDataEvent, AnyRoomAccountDataEvent, RoomAccountDataEventType},
	serde::Raw,
	RoomId, UserId,
};
use serde_json::value::RawValue;

use crate::{globals, Dep};

pub struct Service {
	services: Services,
	db: Data,
}

struct Data {
	roomuserdataid_accountdata: Arc<Map>,
	roomusertype_roomuserdataid: Arc<Map>,
}

struct Services {
	globals: Dep<globals::Service>,
}

impl crate::Service for Service {
	fn build(args: crate::Args<'_>) -> Result<Arc<Self>> {
		Ok(Arc::new(Self {
			services: Services {
				globals: args.depend::<globals::Service>("globals"),
			},
			db: Data {
				roomuserdataid_accountdata: args.db["roomuserdataid_accountdata"].clone(),
				roomusertype_roomuserdataid: args.db["roomusertype_roomuserdataid"].clone(),
			},
		}))
	}

	fn name(&self) -> &str { crate::service::make_name(std::module_path!()) }
}

/// Places one event in the account data of the user and removes the
/// previous entry.
#[allow(clippy::needless_pass_by_value)]
#[implement(Service)]
pub async fn update(
	&self, room_id: Option<&RoomId>, user_id: &UserId, event_type: RoomAccountDataEventType, data: &serde_json::Value,
) -> Result<()> {
	let event_type = event_type.to_string();
	let count = self.services.globals.next_count()?;

	let mut prefix = room_id
		.map(ToString::to_string)
		.unwrap_or_default()
		.as_bytes()
		.to_vec();
	prefix.push(0xFF);
	prefix.extend_from_slice(user_id.as_bytes());
	prefix.push(0xFF);

	let mut roomuserdataid = prefix.clone();
	roomuserdataid.extend_from_slice(&count.to_be_bytes());
	roomuserdataid.push(0xFF);
	roomuserdataid.extend_from_slice(event_type.as_bytes());

	let mut key = prefix;
	key.extend_from_slice(event_type.as_bytes());

	if data.get("type").is_none() || data.get("content").is_none() {
		return Err!(Request(InvalidParam("Account data doesn't have all required fields.")));
	}

	self.db.roomuserdataid_accountdata.insert(
		&roomuserdataid,
		&serde_json::to_vec(&data).expect("to_vec always works on json values"),
	);

	let prev_key = (room_id, user_id, &event_type);
	let prev = self.db.roomusertype_roomuserdataid.qry(&prev_key).await;

	self.db
		.roomusertype_roomuserdataid
		.insert(&key, &roomuserdataid);

	// Remove old entry
	if let Ok(prev) = prev {
		self.db.roomuserdataid_accountdata.remove(&prev);
	}

	Ok(())
}

/// Searches the account data for a specific kind.
#[implement(Service)]
pub async fn get(
	&self, room_id: Option<&RoomId>, user_id: &UserId, kind: RoomAccountDataEventType,
) -> Result<Box<RawValue>> {
	let key = (room_id, user_id, kind.to_string());
	self.db
		.roomusertype_roomuserdataid
		.qry(&key)
		.and_then(|roomuserdataid| self.db.roomuserdataid_accountdata.get(&roomuserdataid))
		.await
		.deserialized()
}

/// Returns all changes to the account data that happened after `since`.
#[implement(Service)]
pub async fn changes_since(
	&self, room_id: Option<&RoomId>, user_id: &UserId, since: u64,
) -> Result<Vec<AnyRawAccountDataEvent>> {
	let mut userdata = HashMap::new();

	let mut prefix = room_id
		.map(ToString::to_string)
		.unwrap_or_default()
		.as_bytes()
		.to_vec();
	prefix.push(0xFF);
	prefix.extend_from_slice(user_id.as_bytes());
	prefix.push(0xFF);

	// Skip the data that's exactly at since, because we sent that last time
	let mut first_possible = prefix.clone();
	first_possible.extend_from_slice(&(since.saturating_add(1)).to_be_bytes());

	self.db
		.roomuserdataid_accountdata
		.raw_stream_from(&first_possible)
		.ignore_err()
		.ready_take_while(move |(k, _)| k.starts_with(&prefix))
		.map(|(k, v)| {
			let v = match room_id {
				None => serde_json::from_slice::<Raw<AnyGlobalAccountDataEvent>>(v)
					.map(AnyRawAccountDataEvent::Global)
					.map_err(|_| Error::bad_database("Database contains invalid account data."))?,
				Some(_) => serde_json::from_slice::<Raw<AnyRoomAccountDataEvent>>(v)
					.map(AnyRawAccountDataEvent::Room)
					.map_err(|_| Error::bad_database("Database contains invalid account data."))?,
			};

			Ok((k.to_owned(), v))
		})
		.ignore_err()
		.ready_for_each(|(kind, data)| {
			userdata.insert(kind, data);
		})
		.await;

	Ok(userdata.into_values().collect())
}
