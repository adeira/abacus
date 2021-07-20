use crate::arangodb::Database;
use crate::arangors::collection::CollectionType;
use crate::migrations::utils::create_collection;

pub async fn migrate(db: &Database) -> anyhow::Result<()> {
    create_collection(&db, "tracking", &CollectionType::Edge, &None).await
}
