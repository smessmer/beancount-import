const INDENT_SIZE: usize = 2;

pub struct BulletPointPrinter<W: LineWriter + Clone> {
    writer: W,
    nesting: usize,
}

impl<W: LineWriter + Clone> BulletPointPrinter<W> {
    pub fn new(writer: W) -> Self {
        Self { writer, nesting: 0 }
    }

    pub fn print_item(&self, message: impl std::fmt::Display) {
        let indent = " ".repeat(self.nesting * INDENT_SIZE);
        self.writer.write_line(&format!("{}• {}", indent, message));
    }

    pub fn indent(&self) -> Self {
        Self {
            writer: self.writer.clone(),
            nesting: self.nesting + 1,
        }
    }
}

impl BulletPointPrinter<StdoutLineWriter> {
    pub fn new_stdout() -> Self {
        Self::new(StdoutLineWriter)
    }
}

pub trait LineWriter {
    fn write_line(&self, line: &str);
}

#[derive(Clone, Copy)]
pub struct StdoutLineWriter;
impl LineWriter for StdoutLineWriter {
    fn write_line(&self, line: &str) {
        println!("{}", line);
    }
}
