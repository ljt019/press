// src/xml_reader.rs

use crate::AppError;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::path::{Path, PathBuf};
use tokio;

pub struct XmlReader<'a> {
    reader: Reader<&'a [u8]>,
    #[allow(dead_code)]
    buf: Vec<u8>,
    current_path: Option<String>,
    current_parts: Vec<(usize, String)>,
    response_txt_content: String,
    in_response_tag: bool,
}

impl<'a> XmlReader<'a> {
    pub fn new(response: &'a str) -> Self {
        let mut reader = Reader::from_str(response);
        // Trim whitespace in text nodes
        reader.config_mut().trim_text(true);

        XmlReader {
            reader,
            buf: Vec::new(),
            current_path: None,
            current_parts: Vec::new(),
            response_txt_content: String::new(),
            in_response_tag: false,
        }
    }

    pub fn handle_file_start(
        &mut self,
        e: &quick_xml::events::BytesStart,
    ) -> Result<(), quick_xml::Error> {
        for attr in e.attributes().with_checks(false) {
            if let Ok(attr) = attr {
                if attr.key.as_ref() == b"path" {
                    let value = attr.unescape_value()?;
                    self.current_path = Some(value.into_owned());
                }
            }
        }
        Ok(())
    }

    pub fn handle_part_start(
        &mut self,
        e: &quick_xml::events::BytesStart,
    ) -> Result<(), quick_xml::Error> {
        if self.in_response_tag {
            // If we are inside <response>, just start a fresh part
            self.current_parts.push((0, String::new()));
        } else {
            // Otherwise, look for an id attribute
            for attr in e.attributes().with_checks(false) {
                if let Ok(attr) = attr {
                    if attr.key.as_ref() == b"id" {
                        let value = attr.unescape_value()?;
                        let part_id = value.parse::<usize>().unwrap_or(0);
                        self.current_parts.push((part_id, String::new()));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn handle_text(&mut self, text: String) {
        // Accumulate text within the last part
        if let Some(last_part) = self.current_parts.last_mut() {
            last_part.1.push_str(&text);
        }
    }

    /// Parse the XML in the AI response and apply changes to each file
    pub async fn process_file(
        &mut self,
        original_paths: &[PathBuf],
        output_directory: &Path,
        auto: bool,
        chunk_size: usize,
    ) -> Result<usize, AppError> {
        let mut saved_files = 0;
        let mut buf = Vec::new();

        loop {
            let event = self.reader.read_event_into(&mut buf);
            match event {
                Ok(Event::Start(ref e)) => match e.name().as_ref() {
                    b"file" | b"new_file" => self.handle_file_start(e)?,
                    b"part" => self.handle_part_start(e)?,
                    b"response" => {
                        self.in_response_tag = true;
                        self.current_parts.clear();
                        // We always keep one "part" open for <response> content
                        self.current_parts.push((0, String::new()));
                    }
                    _ => (),
                },
                Ok(Event::CData(e)) => {
                    let text = String::from_utf8_lossy(&e).into_owned();
                    self.handle_text(text);
                }
                Ok(Event::Text(e)) => {
                    if let Ok(text) = e.unescape() {
                        self.handle_text(text.into_owned());
                    }
                }
                Ok(Event::End(ref e)) => match e.name().as_ref() {
                    b"file" => {
                        // Save or overwrite the file as needed
                        if let Some(path) = self.current_path.take() {
                            let fallback = PathBuf::from(&path);
                            let original_file_path = original_paths
                                .iter()
                                .find(|p| p.to_string_lossy().ends_with(&path))
                                .unwrap_or(&fallback);

                            // Read original file content
                            let original_content =
                                tokio::fs::read_to_string(&original_file_path).await?;

                            // Split original into parts
                            let lines: Vec<&str> = original_content.lines().collect();
                            let mut parts: Vec<String> = if chunk_size == 0 {
                                vec![original_content]
                            } else {
                                lines
                                    .chunks(chunk_size)
                                    .map(|chunk| chunk.join("\n"))
                                    .collect()
                            };

                            // Overwrite only the parts we have in the response
                            for (part_id, content) in self.current_parts.drain(..) {
                                if part_id > 0 && part_id <= parts.len() {
                                    parts[part_id - 1] = content;
                                }
                            }

                            let new_content = parts.join("\n");

                            // Decide final path
                            let output_file_path = if auto {
                                original_file_path.to_path_buf()
                            } else {
                                output_directory.join("code").join(&path)
                            };

                            if let Some(parent) = output_file_path.parent() {
                                tokio::fs::create_dir_all(parent).await?;
                            }

                            tokio::fs::write(&output_file_path, new_content.as_bytes()).await?;
                            saved_files += 1;
                        }
                    }
                    b"new_file" => {
                        // This section handles brand-new files that didn't exist
                        if let Some(path) = self.current_path.take() {
                            let file_path = PathBuf::from(&path);
                            if let Some(parent) = file_path.parent() {
                                tokio::fs::create_dir_all(parent).await?;
                            }
                            let new_content = self
                                .current_parts
                                .drain(..)
                                .map(|(_, content)| content)
                                .collect::<Vec<String>>()
                                .join("\n");
                            tokio::fs::write(&file_path, new_content.as_bytes()).await?;
                            saved_files += 1;
                        }
                    }
                    b"response" => {
                        // End of <response> - store in "response.txt"
                        self.in_response_tag = false;
                        self.response_txt_content = self
                            .current_parts
                            .drain(..)
                            .map(|(_, content)| content)
                            .collect::<Vec<String>>()
                            .join("\n");

                        if !self.response_txt_content.is_empty() {
                            let response_txt_path = output_directory.join("response.txt");
                            tokio::fs::create_dir_all(output_directory).await?;
                            tokio::fs::write(
                                &response_txt_path,
                                self.response_txt_content.as_bytes(),
                            )
                            .await?;
                        }
                    }
                    _ => (),
                },
                Ok(Event::Eof) => {
                    // Reached end of the AI response
                    break;
                }
                Err(e) => {
                    log::error!("Error parsing XML: {:?}", e);
                    break;
                }
                _ => (),
            }
            buf.clear();
        }

        Ok(saved_files)
    }
}
