use std::{future::Future, pin::Pin};

use crate::{
    models::cloud::{
        CloudAuthStatus, CloudFilePlan, CloudFileResult, CloudPartResult, CloudUploadSession,
    },
    AppResult,
};

pub type CloudUploadFuture<'a, T> = Pin<Box<dyn Future<Output = AppResult<T>> + Send + 'a>>;

pub trait CloudUploader: Send + Sync {
    fn auth_status(&self) -> CloudUploadFuture<'_, CloudAuthStatus>;

    fn precreate(&self, plan: CloudFilePlan) -> CloudUploadFuture<'_, CloudUploadSession>;

    fn upload_part<'a>(
        &'a self,
        session: &'a CloudUploadSession,
        part_index: usize,
        bytes: Vec<u8>,
    ) -> CloudUploadFuture<'a, CloudPartResult>;

    fn finish(&self, session: CloudUploadSession) -> CloudUploadFuture<'_, CloudFileResult>;
}
