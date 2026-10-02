use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("kubernetes api error: {0}")]
    Kube(#[from] kube::Error),
    #[error("bundle error: {0}")]
    Bundle(#[from] sandboxpolicy_bundle::BundleError),
    #[error("serialisation error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("object has no namespace")]
    NoNamespace,
}
