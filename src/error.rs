use std::borrow::Cow;


#[derive(Debug)]
pub struct UnsupportedFeatureError {
    pub what_to_use: Cow<'static, str>,
    pub feature: Cow<'static, str>,
}

impl core::fmt::Display for UnsupportedFeatureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Using {} requires the feature `{}` which was not enabled during compilation",
            self.what_to_use,
            self.feature,
        )
    }
}

impl core::error::Error for UnsupportedFeatureError {}
