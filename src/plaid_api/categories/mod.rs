use std::collections::HashMap;

use crate::db::TransactionCategory;

const CATEGORIES_CSV: &str = include_str!("plaid_categories.csv");

pub fn lookup_category(category: &TransactionCategory) -> String {
    // TODO categories() should be const, or at the very least lazy static, to avoid parsing it again and again
    let categories = categories();
    categories
        .get(category)
        .cloned()
        .unwrap_or_else(|| format!("{}.{}", category.primary, category.detailed))
}

fn categories() -> HashMap<TransactionCategory, String> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .delimiter(b',')
        .from_reader(CATEGORIES_CSV.as_bytes());
    reader
        .records()
        .map(|r| {
            let r = r.unwrap();
            (
                TransactionCategory {
                    primary: r.get(0).unwrap().to_string(),
                    detailed: r.get(1).unwrap().to_string(),
                },
                r.get(2).unwrap().to_string(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_categories() {
        let categories = categories();

        assert_eq!(104, categories.len());
        assert_eq!(
            "Pet supplies and pet food",
            categories[&TransactionCategory {
                primary: "GENERAL_MERCHANDISE".to_string(),
                detailed: "GENERAL_MERCHANDISE_PET_SUPPLIES".to_string(),
            }]
        );
        // And a row that has a comma in the description
        assert_eq!(
            "Rental cars, charter buses, and trucks",
            categories[&TransactionCategory {
                primary: "TRAVEL".to_string(),
                detailed: "TRAVEL_RENTAL_CARS".to_string(),
            }],
        );
    }
}
