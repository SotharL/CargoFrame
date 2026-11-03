use cargoframe_net::NetError;

/// Everything that can stop an update, in words for the person.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UpdateError {
    #[error("{0}")]
    Net(#[from] NetError),
    #[error("GitHub did not find a release. If the repository is private, add a read-only token under Updates")]
    NotFound,
    #[error("GitHub did not accept the token. Create a new read-only token for the CargoFrame repository and paste it again")]
    TokenRejected,
    #[error("GitHub says the token cannot read this repository. It needs read access to Contents for the CargoFrame repository")]
    NoAccess,
    #[error("GitHub is limiting how often it can be asked. Try again in a while, or add a token to raise the limit")]
    RateLimited,
    #[error("GitHub answered with something this version of CargoFrame cannot read")]
    BadAnswer,
    #[error("{tag} has no installer for this computer. Only 64-bit Windows and Linux are built")]
    NoInstaller { tag: String },
    #[error(
        "{tag} has no SHA256SUMS.txt, so the installer cannot be checked and will not be installed"
    )]
    NoChecksums { tag: String },
    #[error("SHA256SUMS.txt has no fingerprint for {0}, so it will not be installed")]
    NotListed(String),
    #[error("the downloaded file is not the one the release lists (its fingerprint differs), so it was deleted and nothing was installed")]
    ChecksumMismatch,
    #[error("the release points to a file at an address CargoFrame does not trust: {0}")]
    UntrustedAddress(String),
    #[error("GitHub refused the download ({status}): {message}")]
    DownloadRefused { status: u16, message: String },
    #[error("cannot write the download: {0}")]
    Disk(String),
    #[error("cannot start the installer: {0}")]
    Install(String),
    #[error("updating is not supported on this system")]
    Unsupported,
}

impl UpdateError {
    /// Offer to open the Updates settings, where the token goes.
    pub fn needs_token(&self) -> bool {
        matches!(
            self,
            UpdateError::NotFound | UpdateError::TokenRejected | UpdateError::NoAccess
        )
    }
}
