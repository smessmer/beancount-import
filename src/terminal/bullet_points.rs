use console::StyledObject;

const INDENT_SIZE: usize = 2;

pub struct BulletPointPrinter {
    nesting: usize,
}

impl BulletPointPrinter {
    pub fn new() -> BulletPointPrinter {
        BulletPointPrinter { nesting: 0 }
    }

    pub fn print_item(&self, message: StyledObject<&str>) {
        let indent = " ".repeat(self.nesting * INDENT_SIZE);
        println!("{}• {}", indent, message);
    }

    pub fn indent(&self) -> BulletPointPrinter {
        BulletPointPrinter {
            nesting: self.nesting + 1,
        }
    }
}
