use crate::{database::Database, history::History};
use eyre::Result;
use tantivy::{
    directory::MmapDirectory,
    doc,
    schema::{Field, Schema, FAST, STORED, STRING, TEXT},
    DateTime, Index, IndexWriter,
};

pub fn schema() -> (HistorySchema, Schema) {
    let mut schema_builder = Schema::builder();

    (
        HistorySchema {
            id: schema_builder.add_text_field("id", STRING),
            command: schema_builder.add_text_field("command", TEXT | STORED),
            cwd: schema_builder.add_text_field("cwd", STRING | FAST),
            session: schema_builder.add_text_field("session", STRING | FAST),
            hostname: schema_builder.add_text_field("hostname", STRING | FAST),
            timestamp: schema_builder.add_date_field("timestamp", STORED),
            duration: schema_builder.add_i64_field("duration", STORED),
            exit: schema_builder.add_i64_field("exit", STORED),
        },
        schema_builder.build(),
    )
}

pub struct HistorySchema {
    pub id: Field,
    pub command: Field,
    pub cwd: Field,
    pub session: Field,
    pub hostname: Field,
    pub timestamp: Field,
    pub duration: Field,
    pub exit: Field,
}

pub fn index(schema: Schema) -> Result<Index> {
    let data_dir = atuin_common::utils::data_dir();
    let tantivy_dir = data_dir.join("tantivy");

    fs_err::create_dir_all(&tantivy_dir)?;
    let dir = MmapDirectory::open(tantivy_dir)?;

    Ok(Index::open_or_create(dir, schema)?)
}

pub fn write_history(h: impl IntoIterator<Item = History>) -> Result<()> {
    let (hs, schema) = schema();
    let index = index(schema)?;
    let mut writer = index.writer(3_000_000)?;

    bulk_write_history(&mut writer, &hs, h)?;

    Ok(())
}

pub fn bulk_write_history(
    writer: &mut IndexWriter,
    schema: &HistorySchema,
    h: impl IntoIterator<Item = History>,
) -> Result<()> {
    for h in h {
        write_single_history(writer, schema, h)?;
    }
    writer.commit()?;

    Ok(())
}

fn write_single_history(
    writer: &mut IndexWriter,
    schema: &HistorySchema,
    h: History,
) -> Result<()> {
    let timestamp = DateTime::from_timestamp_millis(h.timestamp.timestamp_millis());
    writer.add_document(doc!(
        schema.id => h.id,
        schema.command => h.command,
        schema.cwd => h.cwd,
        schema.session => h.session,
        schema.hostname => h.hostname,
        schema.timestamp => timestamp,
        schema.duration => h.duration,
        schema.exit => h.exit,
    ))?;

    Ok(())
}

pub async fn refresh(db: &mut impl Database) -> Result<()> {
    let history = db.all_with_count().await?;

    // delete the index
    let data_dir = atuin_common::utils::data_dir();
    let tantivy_dir = dbg!(data_dir.join("tantivy"));
    fs_err::remove_dir_all(tantivy_dir)?;

    tokio::task::spawn_blocking(|| write_history(history.into_iter().map(|(h, _)| h))).await??;

    Ok(())
}
