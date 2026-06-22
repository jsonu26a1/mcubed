use std::fs::File;
use std::io::{Read, Result as IoResult, Seek, SeekFrom, Write};

pub trait IoBackend {
    fn len(&self) -> IoResult<u64>;
    // return std::io::ErrorKind::UnexpectedEof if buffer cannot be filled
    fn read(&mut self, offset: u64, buffer: &mut [u8]) -> IoResult<()>;
    fn write(&mut self, offset: u64, buffer: &[u8]) -> IoResult<()>;
    fn set_len(&mut self, len: u64) -> IoResult<()>;
    fn sync_data(&self) -> IoResult<()>;
}

impl IoBackend for Box<dyn IoBackend> {
    fn len(&self) -> IoResult<u64> {
        (**self).len()
    }
    fn read(&mut self, offset: u64, buffer: &mut [u8]) -> IoResult<()> {
        (**self).read(offset, buffer)
    }
    fn write(&mut self, offset: u64, buffer: &[u8]) -> IoResult<()> {
        (**self).write(offset, buffer)
    }
    fn set_len(&mut self, len: u64) -> IoResult<()> {
        (**self).set_len(len)
    }
    fn sync_data(&self) -> IoResult<()> {
        (**self).sync_data()
    }
}

pub struct FileBackend(pub File);

impl IoBackend for FileBackend {
    fn len(&self) -> IoResult<u64> {
        println!("file.len()");
        Ok(self.0.metadata()?.len())
    }
    fn read(&mut self, offset: u64, buffer: &mut [u8]) -> IoResult<()> {
        // println!("file.read({}, {})", offset, buffer.len());
        self.0.seek(SeekFrom::Start(offset))?;
        self.0.read_exact(buffer)
    }
    fn write(&mut self, offset: u64, buffer: &[u8]) -> IoResult<()> {
        // println!("file.write({}, {})", offset, buffer.len());
        self.0.seek(SeekFrom::Start(offset))?;
        self.0.write_all(buffer)
    }
    fn set_len(&mut self, len: u64) -> IoResult<()> {
        println!("file.set_len({})", len);
        self.0.set_len(len)
    }
    fn sync_data(&self) -> IoResult<()> {
        println!("file.sync_data()");
        self.0.sync_data()
    }
}

pub struct VecBackend(pub Vec<u8>);

impl IoBackend for VecBackend {
    fn len(&self) -> IoResult<u64> {
        println!("vec.len()");
        Ok(self.0.len() as u64)
    }
    fn read(&mut self, offset: u64, mut buffer: &mut [u8]) -> IoResult<()> {
        // println!("vec.read({}, {})", offset, buffer.len());
        let offset = offset as usize;
        let end = offset + buffer.len();
        let _ = buffer.write(&self.0[offset..end]);
        Ok(())
    }
    fn write(&mut self, offset: u64, buffer: &[u8]) -> IoResult<()> {
        // println!("vec.write({}, {})", offset, buffer.len());
        let offset = offset as usize;
        let _ = (&mut self.0[offset..offset + buffer.len()]).write(buffer);
        Ok(())
    }
    fn set_len(&mut self, len: u64) -> IoResult<()> {
        println!("vec.set_len({})", len);
        self.0.resize(len as usize, 0);
        Ok(())
    }
    fn sync_data(&self) -> IoResult<()> {
        println!("vec.sync_data()");
        // no op
        Ok(())
    }
}
