pub trait ArgExtractor {
    fn extract(&self, text: &str) -> Vec<String>;
}
