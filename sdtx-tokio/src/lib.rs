use tokio::fs::File;


pub struct AsyncFile {
    file: File,
}

impl AsyncFile {
    pub fn new(file: File) -> Self {
        AsyncFile { file }
    }

    pub async fn try_clone(&self) -> std::io::Result<Self> {
        Ok(AsyncFile { file: self.file.try_clone().await? })
    }

    pub fn inner(&self) -> &File {
        &self.file
    }

    pub fn inner_mut(&mut self) -> &mut File {
        &mut self.file
    }

    pub fn into_inner(self) -> File {
        self.file
    }
}

impl From<File> for AsyncFile {
    fn from(file: File) -> Self {
        Self::new(file)
    }
}

impl futures::io::AsyncRead for AsyncFile {
    fn poll_read(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>, buf: &mut [u8])
            -> std::task::Poll<std::io::Result<usize>>
    {
        let mut rb = tokio::io::ReadBuf::new(buf);
        let pin = std::pin::Pin::new(&mut self.file);

        futures::ready!(tokio::io::AsyncRead::poll_read(pin, cx, &mut rb))?;

        std::task::Poll::Ready(Ok(rb.filled().len()))
    }
}

impl std::os::unix::io::AsRawFd for AsyncFile {
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        self.file.as_raw_fd()
    }
}


pub type Device = sdtx::Device<AsyncFile>;

pub async fn connect() -> std::io::Result<Device> {
    let file = tokio::fs::File::open(sdtx::DEFAULT_DEVICE_FILE_PATH).await?;
    let file = AsyncFile::from(file);

    Ok(Device::from(file))
}
