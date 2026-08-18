// SPDX-License-Identifier: AGPL-3.0-or-later

use std::cell::RefCell;
use std::rc::Rc;

use tracing_subscriber;

use crate::{Body, Extensions, Hash, Header, Operation, SeqNum, SigningKey, Topic, VerifyingKey};

pub use macro_rules_attribute::apply;

/// Make a function a testing function
///
/// With this macro applied to a function, we get
///
/// - async functions get put into a tokio runtime
/// - Logging is automatically enabled based on the `RUST_LOG` env variable, defaults to `debug`
///
/// # Example
///
/// ```rust
/// #[p2panda_core::test_utils::apply(p2panda_core::test_utils::p2panda_test)]
/// fn my_awesome_test() {
///     // Test body
/// }
/// ```
#[macro_export]
macro_rules! p2panda_test {

    // Entry point for async fn tests
    (
        $( #[$attr:meta] )*
        async fn $name:ident
            (
                $( $generator_variable:ident : $generator_type:ty ),* $(,)?
            )
            $(-> $ret:ty)?
            $body:block
    ) => {
        $crate::test_utils::p2panda_test!(@tracing
            $( #[$attr] )*
            $name
            (
                $( $generator_variable : $generator_type ),*
            )
        {
            #[allow(unused)]
            let ret = $crate::test_utils::p2panda_test!(@add_tokio $body);

            $($crate::test_utils::p2panda_test!(@check_return $ret, ret);)?
        });
    };

    // Entry point for non-async fn tests
    (
        $( #[$attr:meta] )*
        fn $name:ident
            (
                $( $generator_variable:ident : $generator_type:ty ),* $(,)?
            )
            $(-> $ret:ty)?
            $body:block
    ) => {
        $crate::test_utils::p2panda_test!(@tracing $( #[$attr] )* $name
            (
                $( $generator_variable : $generator_type ),*
            )
        {
            #[allow(unused)]
            let ret = { $body };

            $($crate::test_utils::p2panda_test!(@check_return $ret, ret);)?
        });
    };


    // utilities

    // Add tokio runtime around body
    (@add_tokio $body:block) => {
        {
            let rt = $crate::test_utils::create_runtime(concat!("test-runtime-", stringify!($name)))
                .expect("Could not create runtime for tests");

            rt.block_on(async { $body })
        }
    };

    // Add tracing setup around body
    (@tracing
        $( #[$attr:meta] )*
        $name:ident
        (
                $( $generator_variable:ident : $generator_type:ty ),* $(,)?
        )
        $body:block
    ) => {
        $crate::test_utils::p2panda_test!(@function
            $( #[$attr] )*
            $name
            (
                $( $generator_variable : $generator_type ),*
            )

            {
                $crate::test_utils::setup_tracing();

                $body
            }
        );
    };

    // Check the return value of the body
    (@check_return $ret:ty, $value:ident) => {
        let ret: $ret = $value;
        let ret: Result<_, _> = ret;

        if let Err(error) = ret {
            panic!("The test failed: {error:#?}");
        }
    };

    (@function
        $( #[$attr:meta] )*
        $name:ident
        (
                $( $generator_variable:ident : $generator_type:ty ),* $(,)?
        )
        $body:block
    ) => {
        #[test]
        $( #[$attr] )*
        fn $name() {
            $(
                let $generator_variable = <$generator_type as $crate::test_utils::Generatable>::generate();
            )*

            $body
        }
    }
}
pub use p2panda_test;

// Use the
//
///     #[p2panda_core::test_utils::apply(p2panda_core::test_utils::p2panda_test)]
///
/// annotation (see above)
#[doc(hidden)]
pub fn setup_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(
                    "debug".parse().expect("Could not parse built-in EnvFilter"),
                )
                .from_env_lossy(),
        )
        .with_test_writer()
        .try_init();
}

/// Create an async runtime for use with tokio
///
/// The `name` argument is used to give the resulting Runtime this name
#[doc(hidden)]
pub fn create_runtime(name: &'static str) -> Result<tokio::runtime::Runtime, std::io::Error> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .name(name)
        .build()
}

#[derive(Clone, Default)]
pub struct TestLog {
    signing_key: SigningKey,
    backlink: Rc<RefCell<Option<Hash>>>,
    seq_num: Rc<RefCell<SeqNum>>,
    log_id: Topic,
}

impl TestLog {
    pub fn new() -> Self {
        Self {
            signing_key: SigningKey::generate(),
            backlink: Rc::default(),
            seq_num: Rc::default(),
            log_id: Topic::random(),
        }
    }

    pub fn from_signing_key(signing_key: SigningKey) -> Self {
        let mut log = TestLog::new();
        log.signing_key = signing_key;
        log
    }

    pub fn id(&self) -> Topic {
        self.log_id
    }

    pub fn author(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    pub fn operation<E: Extensions>(&self, body: &[u8], extensions: E) -> Operation<E> {
        let body = Body::from(body);

        let mut seq_num = self.seq_num.borrow_mut();
        let mut backlink = self.backlink.borrow_mut();

        let mut header = Header::<E> {
            verifying_key: self.signing_key.verifying_key(),
            version: 1,
            signature: None,
            payload_size: body.size(),
            payload_hash: if body.size() == 0 {
                None
            } else {
                Some(body.hash())
            },
            seq_num: *seq_num,
            backlink: *backlink,
            extensions,
        };
        header.sign(&self.signing_key);

        *backlink = Some(header.hash());
        *seq_num += 1;

        Operation {
            hash: header.hash(),
            header,
            body: if body.size() == 0 { None } else { Some(body) },
        }
    }
}

pub trait Generatable {
    fn generate() -> Self;
}

pub struct RandomTopic(pub Topic);

impl Generatable for RandomTopic {
    fn generate() -> Self {
        RandomTopic(Topic::random())
    }
}

#[cfg(test)]
mod tests {
    use crate::Header;
    use crate::cbor::{decode_cbor, encode_cbor};

    use super::*;
    use super::TestLog;

    #[test]
    fn zero_byte_body() {
        let log = TestLog::new();
        let operation = log.operation(&[], ());
        let bytes = encode_cbor(operation.header()).unwrap();
        assert!(decode_cbor::<Header, _>(&bytes[..]).is_ok());
    }

    #[apply(p2panda_test)]
    fn verify_non_async_test() {}

    #[apply(p2panda_test)]
    async fn verify_async_test() {}

    #[apply(p2panda_test)]
    fn verify_extracting_random_topic(topic: RandomTopic) {
        let _ = topic;
    }

    #[apply(p2panda_test)]
    async fn verify_async_extracting_random_topic(topic: RandomTopic) {
        let _ = topic;
    }
}
