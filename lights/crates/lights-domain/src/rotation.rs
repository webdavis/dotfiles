use crate::ValueError;

#[derive(Debug)]
pub struct Rotation {
    scenes: Vec<String>,
    fallback: String,
}
impl Rotation {
    pub fn new(scenes: Vec<String>, fallback: String) -> Result<Self, ValueError> {
        if scenes.is_empty() {
            return Err(ValueError("empty scene rotation"));
        }
        if !scenes.contains(&fallback) {
            return Err(ValueError("fallback must be in rotation"));
        }
        if scenes
            .iter()
            .any(|s| s.trim().is_empty() || s.chars().any(char::is_control))
        {
            return Err(ValueError("invalid rotation scene name"));
        }
        Ok(Self { scenes, fallback })
    }
    pub fn next(&self, current: Option<&str>) -> &str {
        match self.position(current) {
            Some(index) => &self.scenes[(index + 1) % self.scenes.len()],
            None => &self.fallback,
        }
    }
    pub fn previous(&self, current: Option<&str>) -> &str {
        match self.position(current) {
            Some(index) => &self.scenes[(index + self.scenes.len() - 1) % self.scenes.len()],
            None => &self.fallback,
        }
    }
    fn position(&self, current: Option<&str>) -> Option<usize> {
        current.and_then(|name| self.scenes.iter().position(|scene| scene == name))
    }
}

#[cfg(test)]
mod tests;
