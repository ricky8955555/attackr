use std::path::{Path, PathBuf};

use rocket::{
    http::{hyper::header::CONTENT_DISPOSITION, ContentType},
    response::{Responder, Result as ResponseResult},
    Request, Response,
};
use tokio::{
    fs::File,
    io::{AsyncRead, AsyncSeek, Result as IoResult},
};

pub struct NamedFile<T>
where
    T: AsyncRead + AsyncSeek + Send,
{
    path: PathBuf,
    file: T,
}

impl NamedFile<File> {
    pub async fn open<P: AsRef<Path>>(path: P) -> IoResult<Self> {
        Ok(Self {
            path: path.as_ref().to_path_buf(),
            file: File::open(path).await?,
        })
    }
}

impl<T> NamedFile<T>
where
    T: AsyncRead + AsyncSeek + Send,
{
    pub fn with_name(name: &str, file: T) -> Self
    where
        T: AsyncRead + AsyncSeek + Send,
    {
        Self {
            path: PathBuf::from(name),
            file,
        }
    }
}

impl<'r, T> Responder<'r, 'static> for NamedFile<T>
where
    T: AsyncRead + AsyncSeek + Send + 'static,
{
    fn respond_to(self, _: &'r Request<'_>) -> ResponseResult<'static> {
        let mut response = Response::build().sized_body(None, self.file).ok()?;

        if let Some(ext) = self.path.extension().and_then(|ext| ext.to_str()) {
            if let Some(content_type) = ContentType::from_extension(ext) {
                response.set_header(content_type);
            }
        }

        let content_disposition = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| format!("attachment; filename=\"{}\"", name))
            .unwrap_or_else(|| "attachment".to_string());

        response.set_raw_header(CONTENT_DISPOSITION.as_str(), content_disposition);

        Ok(response)
    }
}
