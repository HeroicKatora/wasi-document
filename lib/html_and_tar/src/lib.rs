use base64::{engine::general_purpose::STANDARD, Engine as _};

mod bytemuck {
    pub fn bytes_of(tar: &super::TarHeader) -> &[u8] {
        let len = core::mem::size_of_val(tar);
        unsafe { &*core::slice::from_raw_parts(tar as *const _ as *const u8, len) }
    }
}

#[derive(Default)]
pub struct TarEngine {
    len: u64,
}

#[repr(C)]
pub struct TarHeader {
    name: [u8; 100],     /*   0 */
    mode: [u8; 8],       /* 100 */
    uid: [u8; 8],        /* 108 */
    gid: [u8; 8],        /* 116 */
    size: [u8; 12],      /* 124 */
    mtime: [u8; 12],     /* 136 */
    chksum: [u8; 8],     /* 148 */
    typeflag: u8,        /* 156 */
    linkname: [u8; 100], /* 157 */
    magic: [u8; 6],      /* 257 */
    version: [u8; 2],    /* 263 */
    uname: [u8; 32],     /* 265 */
    gname: [u8; 32],     /* 297 */
    devmajor: [u8; 8],   /* 329 */
    devminor: [u8; 8],   /* 337 */
    prefix: [u8; 155],   /* 345 */
    /* 500 */
    __padding: [u8; 12],
}

pub struct Entry<'la> {
    /// An ascii name for this file.
    pub name: &'la str,
    /// The data in its raw form. It will be re-encoded to be HTML safe.
    pub data: &'la [u8],
}

pub struct InitialEscape {
    /// What Tar header describes the start of the HTML?
    pub header: TarHeader,
    /// How much of the HTML did we consume?
    pub consumed: usize,
    pub extra: Vec<u8>,
}

pub struct EscapedData {
    pub padding: &'static [u8],
    /// The header entry, which transitions us into TAR semantics.
    pub header: TarHeader,
    /// The file entry which closes the HTML tag with the file name visible to both tar as well as
    /// HTML under appropriate attributes.
    pub file: TarHeader,
    pub data: Vec<u8>,
}

pub struct EscapedSentinel {
    pub padding: &'static [u8],
    /// The header entry, which transitions us into TAR semantics.
    pub header: TarHeader,
}

impl TarEngine {
    /// Mangle the HTML prefix such that we can interpret it as a tar header.
    ///
    /// Must not modify HTML semantics.
    pub fn start_of_file(&mut self, html_head: &[u8], entry_offset: usize) -> InitialEscape {
        assert!(html_head.len() < 94);
        assert_eq!(html_head.last().copied(), Some(b'>'));

        let consumed = html_head.len();
        let all_except_close = html_head.len() - 1;

        let mut this = TarHeader::EMPTY;
        this.name[1..][..all_except_close].copy_from_slice(&html_head[..all_except_close]);
        this.name[1..][all_except_close..][..6].copy_from_slice(b" __A=\"");
        this.prefix[153..].copy_from_slice(b"\">");
        this.typeflag = b'0';

        let tail_len = entry_offset.checked_sub(consumed).unwrap();
        this.assign_size(tail_len);
        this.assign_standards();
        this.assign_checksum();

        self.len += core::mem::size_of::<TarHeader>() as u64;
        self.len += tail_len as u64;

        InitialEscape {
            header: this,
            // extra refers to all the data we are adding. Which isn't anything yet.
            extra: vec![],
            consumed,
        }
    }

    fn qualify_name_for_html_attribute(name: &str) -> &str {
        assert!(name.is_ascii(), "Name must be ascii");

        // FIXME: more permissive than reality.
        assert!(
            name.chars().all(|c| c != '\"'),
            "Name {name} must be HTML compatible without escapes"
        );

        name
    }

    pub fn escaped_insert_base64(&mut self, Entry { name, data }: Entry) -> EscapedData {
        let qualname = Self::qualify_name_for_html_attribute(name);

        let padding = self.pad_to_fit();
        let data = STANDARD.encode(data).into_bytes();

        const START: &[u8] = b"\0<template class=\"wah_polyglot_data\" __A=\"";
        const DATA_START: &[u8] = b"\">";
        const ID: &[u8] = b"\" _wahtml_id=\"";
        const CONT: &[u8] = b"\" __B=\"";

        let mut this = TarHeader::EMPTY;
        this.name[..START.len()].copy_from_slice(START);
        this.assign_size(0);
        this.assign_standards();
        let end_start = this.prefix.len() - ID.len();
        this.prefix[end_start..].copy_from_slice(ID);
        this.assign_checksum();
        self.len += core::mem::size_of::<TarHeader>() as u64;

        let mut file = TarHeader::EMPTY;
        let end_start = this.prefix.len() - DATA_START.len();
        file.name[..qualname.len()].copy_from_slice(qualname.as_bytes());
        file.name[qualname.len()..][1..][..CONT.len()].copy_from_slice(CONT);
        file.assign_size(data.len());
        file.prefix[end_start..].copy_from_slice(DATA_START);
        file.assign_checksum();
        self.len += core::mem::size_of::<TarHeader>() as u64;

        // Followed by the data.
        self.len += data.len() as u64;

        EscapedData {
            padding,
            header: this,
            file,
            data,
        }
    }

    pub fn escaped_continue_base64(&mut self, Entry { name, data }: Entry) -> EscapedData {
        let qualname = Self::qualify_name_for_html_attribute(name);

        let padding = self.pad_to_fit();
        let data = STANDARD.encode(data).into_bytes();

        const START: &[u8] = b"\0</template><template class=\"wah_polyglot_data\" __A=\"";
        const DATA_START: &[u8] = b"\">";
        const ID: &[u8] = b"\" _wahtml_id=\"";
        const CONT: &[u8] = b"\" __B=\"";

        let mut this = TarHeader::EMPTY;
        this.name[..START.len()].copy_from_slice(START);
        this.assign_size(0);
        this.assign_standards();
        let end_start = this.prefix.len() - ID.len();
        this.prefix[end_start..].copy_from_slice(ID);
        this.assign_checksum();
        self.len += core::mem::size_of::<TarHeader>() as u64;

        let mut file = TarHeader::EMPTY;
        let end_start = file.prefix.len() - DATA_START.len();
        file.name[..qualname.len()].copy_from_slice(qualname.as_bytes());
        file.name[qualname.len()..][1..][..CONT.len()].copy_from_slice(CONT);
        file.assign_size(data.len());
        file.prefix[end_start..].copy_from_slice(DATA_START);
        file.assign_checksum();
        self.len += core::mem::size_of::<TarHeader>() as u64;

        // Followed by the data.
        self.len += data.len() as u64;

        EscapedData {
            padding,
            header: this,
            file,
            data,
        }
    }

    /// End a sequence of escaped data, with a particular skip of raw HTML bytes to follow until
    /// the next blocks of such data (again starting as `escaped_insert_base64`).
    pub fn escaped_end(&mut self, skip: usize) -> EscapedSentinel {
        let padding = self.pad_to_fit();
        const START: &[u8] = b"\0</template><template>";
        const END: &[u8] = b"\0</template>";

        let mut this = TarHeader::EMPTY;
        this.name[..START.len()].copy_from_slice(START);
        this.assign_size(skip);
        this.prefix[155 - END.len()..].copy_from_slice(END);
        this.assign_standards();
        this.assign_checksum();

        EscapedSentinel {
            padding,
            header: this,
        }
    }

    /// End a sequence of escaped data with a tar EOF.
    pub fn escaped_eof(&mut self) -> EscapedData {
        EscapedData {
            padding: self.pad_to_fit(),
            header: TarHeader::EMPTY,
            file: TarHeader::EMPTY,
            data: b"</template>".to_vec(),
        }
    }

    /// Outside a data escape block, insert a tar EOF marker.
    pub fn insert_eof(&mut self) -> EscapedData {
        EscapedData {
            padding: self.pad_to_fit(),
            header: TarHeader::EMPTY,
            file: TarHeader::EMPTY,
            data: vec![],
        }
    }

    fn pad_to_fit(&mut self) -> &'static [u8] {
        static POTENTIAL_PADDING: [u8; 512] = [0; 512];
        let pad = self.len.next_multiple_of(512) - self.len;
        self.len += pad;
        &POTENTIAL_PADDING[..pad as usize]
    }
}

impl TarHeader {
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(self)
    }

    pub fn assign_standards(&mut self) {
        self.mode.copy_from_slice(b"0000644\0");
        self.uid.copy_from_slice(b"0001750\0");
        self.gid.copy_from_slice(b"0001750\0");
        self.mtime.copy_from_slice(b"14707041774\0");
        self.magic = *b"ustar ";
        self.version = *b" \0";
        self.uname[..7].copy_from_slice(b"nobody\0");
        self.gname[..7].copy_from_slice(b"nobody\0");
    }

    pub fn assign_checksum(&mut self) {
        let mut acc = 0u32;

        for by in &mut self.chksum {
            *by = b' ';
        }

        for &by in self.as_bytes() {
            acc += u32::from(by);
        }

        let bytes = format!("{acc:06o}\0 ");
        self.chksum.copy_from_slice(bytes.as_bytes());
    }

    fn assign_size(&mut self, size: usize) {
        let bytes = format!("{size:011o}\0");
        // Note: this is numeric, so can not contain a closing quote.
        self.size.copy_from_slice(bytes.as_bytes());
    }

    const EMPTY: Self = TarHeader {
        name: [0; 100],
        mode: [0; 8],
        uid: [0; 8],
        gid: [0; 8],
        size: [0; 12],
        mtime: [0; 12],
        chksum: [0; 8],
        typeflag: 0,
        linkname: [0; 100],
        magic: [0; 6],
        version: [0; 2],
        uname: [0; 32],
        gname: [0; 32],
        devmajor: [0; 8],
        devminor: [0; 8],
        prefix: [0; 155],
        __padding: [0; 12],
    };
}
