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
/// use p2panda_core::test_utils::apply;
/// use p2panda_core::p2panda_test;
///
/// #[apply(p2panda_test)]
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
        #[test]
        fn $name() {
            $crate::test_utils::setup_tracing();

            $( #[$attr] )*
            async fn inner(
                $( $generator_variable : $generator_type ),*
            ) $( -> $ret )? {
                $body
            }

            let rt = $crate::test_utils::create_runtime(concat!("test-runtime-", stringify!($name)))
                .expect("Could not create runtime for tests");

            rt.block_on(async move {
                let _test_result = $crate::test_utils::run_async_test(inner).await;
            });
        }
    };

    // Entry point for non-async fn tests
    //(
    //    $( #[$attr:meta] )*
    //    fn $name:ident
    //        (
    //            $( $generator_variable:ident : $generator_type:ty ),* $(,)?
    //        )
    //        $(-> $ret:ty)?
    //        $body:block
    //) => {
    //    $crate::test_utils::p2panda_test!(@tracing $( #[$attr] )* $name
    //        (
    //            $( $generator_variable : $generator_type ),*
    //        )
    //    {
    //        #[allow(unused)]
    //        let ret = { $body };

    //        $($crate::test_utils::p2panda_test!(@check_return $ret, ret);)?
    //    });
    //};


    //// utilities

    //// Add tokio runtime around body
    //(@add_tokio $body:block) => {
    //    {
    //        let rt = $crate::test_utils::create_runtime(concat!("test-runtime-", stringify!($name)))
    //            .expect("Could not create runtime for tests");

    //        rt.block_on(async { $body })
    //    }
    //};

    //// Add tracing setup around body
    //(@tracing
    //    $( #[$attr:meta] )*
    //    $name:ident
    //    (
    //            $( $generator_variable:ident : $generator_type:ty ),* $(,)?
    //    )
    //    $body:block
    //) => {
    //    $crate::test_utils::p2panda_test!(@function
    //        $( #[$attr] )*
    //        $name
    //        (
    //            $( $generator_variable : $generator_type ),*
    //        )

    //        {
    //            $crate::test_utils::setup_tracing();

    //            $body
    //        }
    //    );
    //};

    //// Check the return value of the body
    //(@check_return $ret:ty, $value:ident) => {
    //    let ret: $ret = $value;
    //    let ret: Result<_, _> = ret;

    //    if let Err(error) = ret {
    //        panic!("The test failed: {error:#?}");
    //    }
    //};

    //(@function
    //    $( #[$attr:meta] )*
    //    $name:ident
    //    (
    //            $( $generator_variable:ident : $generator_type:ty ),* $(,)?
    //    )
    //    $body:block
    //) => {
    //    #[test]
    //    $( #[$attr] )*
    //    fn $name() {
    //        $(
    //            let $generator_variable = <$generator_type as $crate::test_utils::Generatable>::generate();
    //        )*

    //        $body
    //    }
    //}
}
pub use p2panda_test;

pub async fn run_async_test<F, R>(testfunction: F) -> R
where
    F: TestFunc<Return = R>,
    R: TestResult,
{
    testfunction.call_test_function().await
}

pub trait TestFunc {
    type Return: TestResult;
    async fn call_test_function(&self) -> Self::Return;
}

#[rustfmt::skip]
macro_rules! all_the_tuples {
    ($name:ident) => {
        $name!([], T1);
        $name!([T1], T2);
        $name!([T1, T2], T3);
        $name!([T1, T2, T3], T4);
        $name!([T1, T2, T3, T4], T5);
        $name!([T1, T2, T3, T4, T5], T6);
        $name!([T1, T2, T3, T4, T5, T6], T7);
        $name!([T1, T2, T3, T4, T5, T6, T7], T8);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8], T9);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9], T10);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10], T11);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11], T12);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12], T13);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13], T14);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14], T15);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15], T16);
    };
}

pub trait Generate
where
    Self: Sized + Send,
{
    async fn generate() -> Self;
}

macro_rules! impl_testfunc {
    (
        [$($ty:ident),*], $last:ident
    ) => {
        impl<$($ty,)* $last> TestFunc for ($($ty,)* $last,)
        where
            $( $ty: Generate, )*
            $last: Generate,
        {
            type Return = Box<dyn TestResult>;

            async fn call_test_function(&self) -> Result<(), Self::Return> {
                (self)(
                    $(
                        $ty::generate(),
                    )*

                    $last::generate(),
                ).map_err(Box::new)
            }
        }
    }
}

all_the_tuples!(impl_testfunc);

pub trait TestResult {}

impl TestResult for () {}

impl<E> TestResult for std::result::Result<(), E> where E: std::error::Error {}

impl<T: TestResult + 'static> TestResult for Box<T> {}

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

    use super::TestLog;
    use super::*;

    #[test]
    fn zero_byte_body() {
        let log = TestLog::new();
        let operation = log.operation(&[], ());
        let bytes = encode_cbor(operation.header()).unwrap();
        assert!(decode_cbor::<Header, _>(&bytes[..]).is_ok());
    }

    // #[apply(p2panda_test)]
    // fn verify_non_async_test() {}

    #[apply(p2panda_test)]
    async fn verify_async_test() {}

    // #[apply(p2panda_test)]
    // fn verify_extracting_random_topic(topic: RandomTopic) {
    //     let _ = topic;
    // }

    // #[apply(p2panda_test)]
    // async fn verify_async_extracting_random_topic(topic: RandomTopic) {
    //     let _ = topic;
    // }
}
