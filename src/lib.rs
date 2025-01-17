// Read and write vorbiscomment metadata

extern crate byteorder;
extern crate lewton;
extern crate ogg;

use lewton::header::HeaderReadError;
//use lewton::header::CommentHeader;
use ogg::writing::PacketWriteEndInfo;
use ogg::{OggReadError, Packet, PacketReader, PacketWriter};
use std::convert::TryInto;
use std::io::{self, Cursor, Read, Seek};

use thiserror::Error;

pub type CommentHeader = lewton::header::CommentHeader;

#[derive(Error, Debug)]
#[error("failed to make vorbis comment")]
pub struct VorbisMakeCommentError();

#[derive(Error, Debug)]
pub enum VorbisReadCommentError {
    #[error("failed to read vorbis file")]
    FailedReadOggFile(#[from] OggReadError),
    #[error("failed to read vorbis header")]
    FailedReadHeader(#[from] HeaderReadError),
}

#[derive(Error, Debug)]
pub enum VorbisReplaceCommentError {
    #[error("failed to write vorbis file")]
    FailedReadOggFile(#[from] io::Error),
}

//type VorbisComments = CommentHeader;
pub trait VorbisComments {
    fn from(vendor: String, comment_list: Vec<(String, String)>) -> CommentHeader;
    fn new() -> CommentHeader;
    fn get_tag_names(&self) -> Vec<String>;
    fn get_tag_single(&self, tag: &str) -> Option<String>;
    fn get_tag_multi(&self, tag: &str) -> Vec<String>;
    fn clear_tag(&mut self, tag: &str);
    fn add_tag_single(&mut self, tag: &str, value: &str);
    fn add_tag_multi(&mut self, tag: &str, values: &Vec<&str>);
    fn get_vendor(&self) -> String;
    fn set_vendor(&mut self, vend: &str);
}

impl VorbisComments for CommentHeader {
    fn from(vendor: String, comment_list: Vec<(String, String)>) -> CommentHeader {
        CommentHeader {
            vendor,
            comment_list,
        }
    }

    fn new() -> CommentHeader {
        CommentHeader {
            vendor: "".to_string(),
            comment_list: Vec::new(),
        }
    }

    fn get_tag_names(&self) -> Vec<String> {
        let mut names = self
            .comment_list
            .iter()
            .map(|comment| comment.0.to_lowercase())
            .collect::<Vec<String>>();
        names.sort_unstable();
        names.dedup();
        names
    }

    fn get_tag_single(&self, tag: &str) -> Option<String> {
        let tags = self.get_tag_multi(tag);
        let result = if tags.len() > 0 {
            Some(tags[0].to_string())
        } else {
            None
        };
        result
    }

    fn get_tag_multi(&self, tag: &str) -> Vec<String> {
        self.comment_list
            .clone()
            .iter()
            .filter(|comment| comment.0.to_lowercase() == tag.to_string().to_lowercase())
            .map(|comment| comment.1.clone())
            .collect::<Vec<String>>()
    }

    fn clear_tag(&mut self, tag: &str) {
        self.comment_list
            .retain(|comment| comment.0.to_lowercase() != tag.to_string().to_lowercase());
    }

    fn add_tag_single(&mut self, tag: &str, value: &str) {
        self.comment_list
            .push((tag.to_string().to_lowercase(), value.to_string()));
    }

    fn add_tag_multi(&mut self, tag: &str, values: &Vec<&str>) {
        for value in values.iter() {
            self.comment_list
                .push((tag.to_string().to_lowercase(), value.to_string()));
        }
    }

    fn get_vendor(&self) -> String {
        self.vendor.to_string()
    }

    fn set_vendor(&mut self, vend: &str) {
        self.vendor = vend.to_string();
    }
}

pub fn safe_make_comment_header(header: &CommentHeader) -> Result<Vec<u8>, VorbisMakeCommentError> {
    //Signature
    let start = [3u8, 118, 111, 114, 98, 105, 115];

    //Vendor number of bytes as u32
    let vendor = header.vendor.as_bytes();
    let vendor_len: u32 = vendor
        .len()
        .try_into()
        .map_err(|_| VorbisMakeCommentError())?;

    //end byte
    let end: u8 = 1;

    let mut new_packet: Vec<u8> = vec![];

    //write start
    new_packet.extend(start.iter().cloned());

    //write vendor
    new_packet.extend(vendor_len.to_le_bytes().iter().cloned());
    new_packet.extend(vendor.iter().cloned());

    //write number of comments
    let comment_nbr: u32 = header
        .comment_list
        .len()
        .try_into()
        .map_err(|_| VorbisMakeCommentError())?;
    new_packet.extend(comment_nbr.to_le_bytes().iter().cloned());

    let mut commentstrings: Vec<String> = vec![];
    //write each comment
    for comment in header.comment_list.iter() {
        commentstrings.push(format!("{}={}", comment.0, comment.1));
        //let commenstrings.last().as_bytes();
        let comment_len: u32 = commentstrings
            .last()
            .ok_or_else(|| VorbisMakeCommentError())?
            .as_bytes()
            .len()
            .try_into()
            .map_err(|_| VorbisMakeCommentError())?;
        new_packet.extend(comment_len.to_le_bytes().iter().cloned());
        new_packet.extend(
            commentstrings
                .last()
                .ok_or_else(|| VorbisMakeCommentError())?
                .as_bytes()
                .iter()
                .cloned(),
        );
    }
    new_packet.push(end);
    //println!("{:?}",new_packet);
    Ok(new_packet)
}

pub fn make_comment_header(header: &CommentHeader) -> Vec<u8> {
    safe_make_comment_header(header).unwrap()
}

pub fn safe_read_comment_header<T: Read + Seek>(
    f_in: T,
) -> Result<CommentHeader, VorbisReadCommentError> {
    let mut reader = PacketReader::new(f_in);

    let packet: Packet = reader.read_packet_expected()?;
    let stream_serial = packet.stream_serial();

    let mut packet: Packet = reader.read_packet_expected()?;
    //println!("{:?}",packet.data);
    while packet.stream_serial() != stream_serial {
        packet = reader.read_packet_expected()?;
        //println!("{:?}",packet.data);
    }
    let comment_hdr = lewton::header::read_header_comment(&packet.data)?; //println!("{:?}", comment_hdr);
    Ok(comment_hdr)
}

pub fn read_comment_header<T: Read + Seek>(f_in: T) -> CommentHeader {
    return safe_read_comment_header(f_in).unwrap();
}

pub fn safe_replace_comment_header<T: Read + Seek>(
    f_in: T,
    new_header: CommentHeader,
) -> Result<Cursor<Vec<u8>>, VorbisReplaceCommentError> {
    let new_comment_data = make_comment_header(&new_header);

    let f_out_ram: Vec<u8> = vec![];
    let mut f_out = Cursor::new(f_out_ram);

    let mut reader = PacketReader::new(f_in);
    let mut writer = PacketWriter::new(&mut f_out);

    let mut header_done = false;
    loop {
        let rp = reader.read_packet();
        match rp {
            Ok(r) => {
                match r {
                    Some(mut packet) => {
                        let inf = if packet.last_in_stream() {
                            PacketWriteEndInfo::EndStream
                        } else if packet.last_in_page() {
                            PacketWriteEndInfo::EndPage
                        } else {
                            PacketWriteEndInfo::NormalPacket
                        };
                        if !header_done {
                            let comment_hdr = lewton::header::read_header_comment(&packet.data);
                            match comment_hdr {
                                Ok(_hdr) => {
                                    // This is the packet to replace
                                    packet.data = new_comment_data.clone();
                                    header_done = true;
                                }
                                Err(_error) => {}
                            }
                        }
                        let lastpacket = packet.last_in_stream() && packet.last_in_page();
                        let stream_serial = packet.stream_serial();
                        let absgp_page = packet.absgp_page();
                        writer.write_packet(packet.data, stream_serial, inf, absgp_page)?;
                        if lastpacket {
                            break;
                        }
                    }
                    // End of stream
                    None => break,
                }
            }
            Err(error) => {
                println!("Error reading packet: {:?}", error);
                break;
            }
        }
    }
    f_out.seek(std::io::SeekFrom::Start(0))?;
    Ok(f_out)
}

pub fn replace_comment_header<T: Read + Seek>(
    f_in: T,
    new_header: CommentHeader,
) -> Cursor<Vec<u8>> {
    safe_replace_comment_header(f_in, new_header).unwrap()
}
