#[derive(Default, Debug)]
pub struct Logger {
    is_debug: bool,
}

impl Logger {
    pub fn new(is_debug: bool) -> Logger{
        Logger { is_debug}
    }

    pub fn debug(&self, msg: &str) {
        if self.is_debug {
            eprintln!("{}", msg);
        }
    }
}