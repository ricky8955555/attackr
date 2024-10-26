use std::path::{self, Path, PathBuf};

use futures_util::{future::BoxFuture, FutureExt};
use tokio::{
    fs::{self, DirEntry, ReadDir},
    io,
};

#[allow(dead_code)]
pub struct WalkDir {
    pub root: PathBuf,
    pub follow_symlink: bool,
    inner: Option<Box<WalkDir>>,
    read_dir: ReadDir,
}

impl WalkDir {
    pub async fn new<P: AsRef<Path>>(path: P, follow_symlink: bool) -> io::Result<Self> {
        let read_dir = fs::read_dir(&path).await?;

        Ok(Self {
            root: path.as_ref().to_path_buf(),
            follow_symlink,
            inner: None,
            read_dir,
        })
    }

    pub fn next_entry(&mut self) -> BoxFuture<io::Result<Option<DirEntry>>> {
        async move {
            if let Some(inner) = self.inner.as_mut() {
                if let Some(entry) = inner.next_entry().await? {
                    return Ok(Some(entry));
                }

                self.inner = None;
            }

            let entry = self.read_dir.next_entry().await?;

            if entry.is_none() {
                return Ok(None);
            }

            let entry = entry.unwrap();
            let metadata = entry.metadata().await?;

            let symlink = if self.follow_symlink && metadata.is_symlink() {
                Some(fs::read_link(entry.path()).await?)
            } else {
                None
            };

            if metadata.is_dir() || symlink.is_some_and(|path| path.is_dir()) {
                if self.inner.is_some() {
                    unreachable!("inner should be None here.");
                }

                self.inner = Some(Box::new(
                    WalkDir::new(entry.path(), self.follow_symlink).await?,
                ));

                return self.next_entry().await;
            }

            Ok(Some(entry))
        }
        .boxed()
    }
}

fn relative_to<P, Q>(path: P, root: Q) -> io::Result<PathBuf>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    let mut comps = path.as_ref().components();

    for root_comp in root.as_ref().components() {
        let comp = comps.next();

        if comp.is_none() || root_comp != comp.unwrap() {
            return Err(io::Error::other(
                format!("{:?} is not in the subpath of {:?} OR one path is relative and the other is absolute.",
                path.as_ref(), root.as_ref()
            )));
        }
    }

    Ok(comps.as_path().to_path_buf())
}

#[derive(Debug, Clone)]
pub struct CopyOptions {
    pub follow_symlink: bool,
    pub erase: bool,
    pub overwrite: bool,
    pub copy_inside: bool,
}

#[allow(dead_code)]
impl CopyOptions {
    pub fn new() -> Self {
        Self {
            follow_symlink: true,
            erase: false,
            overwrite: false,
            copy_inside: false,
        }
    }

    pub fn follow_symlink(mut self, value: bool) -> Self {
        self.follow_symlink = value;
        self
    }

    pub fn erase(mut self, value: bool) -> Self {
        self.erase = value;
        self
    }

    pub fn overwrite(mut self, value: bool) -> Self {
        self.overwrite = value;
        self
    }

    pub fn copy_inside(mut self, value: bool) -> Self {
        self.copy_inside = value;
        self
    }
}

impl Default for CopyOptions {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn copy_dir<P, Q>(source: P, target: Q, options: &CopyOptions) -> io::Result<()>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    let target = if options.copy_inside {
        target.as_ref().join(
            path::absolute(source.as_ref())?
                .file_name()
                .expect("file_name should be available here."),
        )
    } else {
        target.as_ref().to_path_buf()
    };

    if options.erase && target.exists() {
        fs::remove_dir_all(&target).await?;
    }

    let mut walk_dir = WalkDir::new(&source, options.follow_symlink).await?;

    while let Some(entry) = walk_dir.next_entry().await? {
        let path = relative_to(entry.path(), &source)?;
        let target = target.join(&path);

        if target.exists() && !options.overwrite {
            continue;
        }

        if let Some(parent) = target.parent() {
            if !parent.exists() {
                fs::create_dir_all(&parent).await?;
            }
        }

        let source = entry.path();

        fs::copy(&source, &target).await?;
    }

    Ok(())
}
