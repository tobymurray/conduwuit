use clap::Subcommand;
use conduit::Result;
use ruma::{events::room::message::RoomMessageEventContent, ServerName};

use crate::Command;

#[derive(Debug, Subcommand)]
/// All the getters and iterators from src/database/key_value/globals.rs
pub(crate) enum GlobalsCommand {
	DatabaseVersion,

	CurrentCount,

	LastCheckForUpdatesId,

	/// - This returns an empty `Ok(BTreeMap<..>)` when there are no keys found
	///   for the server.
	SigningKeysFor {
		origin: Box<ServerName>,
	},
}

/// All the getters and iterators from src/database/key_value/globals.rs
pub(super) async fn process(subcommand: GlobalsCommand, context: &Command<'_>) -> Result<RoomMessageEventContent> {
	let services = context.services;

	match subcommand {
		GlobalsCommand::DatabaseVersion => {
			let timer = tokio::time::Instant::now();
			let results = services.globals.db.database_version().await;
			let query_time = timer.elapsed();

			Ok(RoomMessageEventContent::notice_markdown(format!(
				"Query completed in {query_time:?}:\n\n```rs\n{results:#?}\n```"
			)))
		},
		GlobalsCommand::CurrentCount => {
			let timer = tokio::time::Instant::now();
			let results = services.globals.db.current_count();
			let query_time = timer.elapsed();

			Ok(RoomMessageEventContent::notice_markdown(format!(
				"Query completed in {query_time:?}:\n\n```rs\n{results:#?}\n```"
			)))
		},
		GlobalsCommand::LastCheckForUpdatesId => {
			let timer = tokio::time::Instant::now();
			let results = services.updates.last_check_for_updates_id().await;
			let query_time = timer.elapsed();

			Ok(RoomMessageEventContent::notice_markdown(format!(
				"Query completed in {query_time:?}:\n\n```rs\n{results:#?}\n```"
			)))
		},
		GlobalsCommand::SigningKeysFor {
			origin,
		} => {
			let timer = tokio::time::Instant::now();
			let results = services.server_keys.verify_keys_for(&origin).await;
			let query_time = timer.elapsed();

			Ok(RoomMessageEventContent::notice_markdown(format!(
				"Query completed in {query_time:?}:\n\n```rs\n{results:#?}\n```"
			)))
		},
	}
}
