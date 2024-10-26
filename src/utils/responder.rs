use std::path::{Path, PathBuf};

use rocket::{
    http::{hyper::header::CONTENT_DISPOSITION, ContentType},
    response::{Responder, Result as ResponseResult},
    Request,
};
use tokio::{fs::File, io::Result as IoResult};

pub struct NamedFile {
    path: PathBuf,
    file: File,
}

impl NamedFile {
    pub async fn open<P: AsRef<Path>>(path: P) -> IoResult<Self> {
        Ok(Self {
            path: path.as_ref().to_path_buf(),
            file: File::open(path).await?,
        })
    }
}

impl<'r> Responder<'r, 'static> for NamedFile {
    fn respond_to(self, request: &'r Request<'_>) -> ResponseResult<'static> {
        let mut response = self.file.respond_to(request)?;

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
