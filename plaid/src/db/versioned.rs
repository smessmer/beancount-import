use serde::{Deserialize, Serialize};

use super::database::{DatabaseV1, DatabaseV2};

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(PartialEq, Eq, Debug))]
pub enum VersionedDatabase {
    V1(DatabaseV1),
    V2(DatabaseV2),
}
