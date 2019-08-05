use ed25519_dalek::Keypair;
use sha2::Sha512;

/// Trait with Ed25519 functions
pub trait Ed25519CryptoBox {
    /// Signs provided data with Ed25519 algrotihm
    fn sign_ed25519(&mut self, data: &[u8]) -> CryptoBoxResult<Vec<u8>>;
}

/// Trait for Ed25519 keys fetching
pub trait Ed25519KeyProvider {
    fn get_ed25519_key_pair(&mut self) -> CryptoBoxResult<Vec<u8>>;
}


/// Struct implementing `Ed25519CryptoBox` with local keys
pub struct Ed25519KeyHoldingCryptoBox {
    key_pair: Keypair
}

impl Ed25519KeyHoldingCryptoBox {
    /// Creates srtuct instance with key pair data
    pub fn new(key_pair: &[u8]) -> CryptoBoxResult<Ed25519KeyHoldingCryptoBox> {
        let key_pair = Keypair::from_bytes(key_pair)
            .map_err(|err| CryptoBoxError::from(CryptoBoxErrorKind::SignatureError(
                format!("Keypair construction failed: {}", err))))?;

        Ok(Ed25519KeyHoldingCryptoBox { key_pair })
    }
}

impl Ed25519CryptoBox for Ed25519KeyHoldingCryptoBox {
    fn sign_ed25519(&mut self, data: &[u8]) -> CryptoBoxResult<Vec<u8>> {
        Ok(self.key_pair.sign::<Sha512>(data).to_bytes().to_vec())
    }
}


/// Struct implementing `Ed25519CryptoBox` with `Ed25519KeyProvider` keys fetcher
pub struct Ed25519KeyDerivingCryptoBox {
    key_pair: Option<Keypair>,
    key_provider: Box<Ed25519KeyProvider>
}

impl Ed25519KeyDerivingCryptoBox {
    /// Creates srtuct instance with `Ed25519KeyProvider`
    pub fn new(key_provider: Box<Ed25519KeyProvider>) -> CryptoBoxResult<Ed25519KeyDerivingCryptoBox> {
        Ok(Ed25519KeyDerivingCryptoBox { key_pair: None, key_provider })
    }
}

impl Ed25519CryptoBox for Ed25519KeyDerivingCryptoBox {
    fn sign_ed25519(&mut self, data: &[u8]) -> CryptoBoxResult<Vec<u8>> {
        let key_pair = if let Some(pair) = &self.key_pair {
            pair
        } else {
            let key_pair_data = self.key_provider.get_ed25519_key_pair()?;

            self.key_pair = Some(
                Keypair::from_bytes(&key_pair_data)
                    .map_err(|err| CryptoBoxError::from(CryptoBoxErrorKind::SignatureError(
                        format!("Keypair construction failed: {}", err))))?
            );

            self.key_pair.as_ref().unwrap()
        };

        Ok(key_pair.sign::<Sha512>(data).to_bytes().to_vec())
    }
}

error_chain! {
    types {
        CryptoBoxError, CryptoBoxErrorKind, CryptoBoxResultExt, CryptoBoxResult;
    }

    errors {
        SignatureError(msg: String) {
            description("Signature error"),
            display("Signature error: {}", msg)
        }
        KeyDerivationError(msg: String) {
            description("Key derivation error"),
            display("Key derivation error: {}", msg)
        }
    }
}