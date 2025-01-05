use crate::errors::AppError;
use quick_xml::events::Event;
use quick_xml::{Reader, Writer};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tokio;

pub struct XmlParser<'a> {
    reader: Reader<&'a [u8]>,
    #[allow(dead_code)]
    buf: Vec<u8>,
    current_path: Option<String>,
    current_parts: Vec<(usize, String)>,
    response_txt_content: String,
    in_response_tag: bool,
}

impl<'a> XmlParser<'a> {
    pub fn new(response: &'a str) -> Self {
        let mut reader = Reader::from_str(response);
        reader.config_mut().trim_text(true);

        XmlParser {
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
                } else if attr.key.as_ref() == b"parts" {
                    let value = attr.unescape_value()?;
                    let part_ids: Vec<u32> = value
                        .split(',')
                        .filter_map(|s| s.parse::<u32>().ok())
                        .collect();
                    self.current_parts = part_ids
                        .into_iter()
                        .map(|id| (id as usize, String::new()))
                        .collect();
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
            self.current_parts.push((0, String::new()));
        } else {
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
        if let Some(last_part) = self.current_parts.last_mut() {
            last_part.1.push_str(&text);
        }
    }

    pub fn filter_preprocessed_prompt(
        &mut self,
        preprocessed_prompt: &str,
        parts_to_edit: &HashMap<String, Vec<usize>>,
    ) -> Result<String, AppError> {
        let mut reader = Reader::from_str(preprocessed_prompt);
        reader.config_mut().trim_text(true);

        let mut writer = Writer::new(Cursor::new(Vec::new()));
        let mut buf = Vec::new();
        let mut current_file_path = None;
        let mut current_file_parts = Vec::new();
        let mut in_requested_part = false;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    if e.name().as_ref() == b"file" {
                        // Reset state for a new file
                        current_file_path = None;
                        current_file_parts.clear();

                        // Check if this file has parts to edit
                        for attr in e.attributes().with_checks(false) {
                            if let Ok(attr) = attr {
                                if attr.key.as_ref() == b"path" {
                                    let path = attr.unescape_value()?.into_owned();
                                    if let Some(parts) = parts_to_edit.get(&path) {
                                        current_file_path = Some(path);
                                        current_file_parts = parts.clone();
                                        writer.write_event(Event::Start(e.clone()))?;
                                    }
                                }
                            }
                        }
                    } else if e.name().as_ref() == b"part" {
                        // Check if this part should be included
                        if let Some(_) = &current_file_path {
                            for attr in e.attributes().with_checks(false) {
                                if let Ok(attr) = attr {
                                    if attr.key.as_ref() == b"id" {
                                        let id = attr
                                            .unescape_value()?
                                            .parse::<usize>()
                                            .expect("Invalid part ID");
                                        if current_file_parts.contains(&id) {
                                            writer.write_event(Event::Start(e.clone()))?;
                                            in_requested_part = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(Event::End(e)) => {
                    if e.name().as_ref() == b"file" {
                        if current_file_path.is_some() {
                            writer.write_event(Event::End(e))?;
                            current_file_path = None;
                            current_file_parts.clear();
                        }
                    } else if e.name().as_ref() == b"part" {
                        if in_requested_part {
                            writer.write_event(Event::End(e))?;
                            in_requested_part = false;
                        }
                    }
                }
                Ok(Event::Text(e)) => {
                    if in_requested_part {
                        writer.write_event(Event::Text(e))?;
                    }
                }
                Ok(Event::CData(e)) => {
                    if in_requested_part {
                        writer.write_event(Event::CData(e))?;
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(AppError::XmlError(e)),
                _ => {}
            }
            buf.clear();
        }

        let result = writer.into_inner().into_inner();
        let filtered_prompt = String::from_utf8(result).expect("Invalid UTF-8 in filtered prompt");
        Ok(filtered_prompt)
    }

    /// Parses the `<parts_to_edit>` section of the preprocessed prompt and returns a HashMap
    /// mapping file paths to their associated part IDs.
    pub fn parse_parts_to_edit(
        &mut self,
        preprocessed_prompt: &str,
    ) -> Result<HashMap<String, Vec<usize>>, AppError> {
        let mut reader = Reader::from_str(preprocessed_prompt);
        reader.config_mut().trim_text(true);

        let mut parts_to_edit = HashMap::new();
        let mut buf = Vec::new();
        let mut current_path = None;
        let mut current_parts = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    if e.name().as_ref() == b"file" {
                        for attr in e.attributes().with_checks(false) {
                            if let Ok(attr) = attr {
                                match attr.key.as_ref() {
                                    b"path" => {
                                        current_path = Some(attr.unescape_value()?.into_owned());
                                    }
                                    b"parts" => {
                                        let parts_str = attr.unescape_value()?;
                                        current_parts = parts_str
                                            .split(',')
                                            .filter_map(|s| s.parse::<usize>().ok())
                                            .collect();
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                Ok(Event::End(e)) => {
                    if e.name().as_ref() == b"file" {
                        if let Some(path) = current_path.take() {
                            parts_to_edit.insert(path, current_parts.clone());
                        }
                        current_parts.clear();
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(AppError::XmlError(e));
                }
                _ => {}
            }
            buf.clear();
        }

        Ok(parts_to_edit)
    }

    /// Process the XML in the AI response and apply changes to each file
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
                        self.current_parts.push((0, String::new()));
                    }
                    b"parts_to_edit" => {
                        self.current_path = None;
                        self.current_parts.clear();
                    }
                    b"preprocessor_prompt" => {
                        self.current_path = None;
                        self.current_parts.clear();
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
                        if let Some(path) = self.current_path.take() {
                            let fallback = PathBuf::from(&path);
                            let original_file_path = original_paths
                                .iter()
                                .find(|p| p.to_string_lossy().ends_with(&path))
                                .unwrap_or(&fallback);

                            let original_content =
                                tokio::fs::read_to_string(&original_file_path).await?;

                            let lines: Vec<&str> = original_content.lines().collect();
                            let mut parts: Vec<String> = if chunk_size == 0 {
                                vec![original_content]
                            } else {
                                lines
                                    .chunks(chunk_size)
                                    .map(|chunk| chunk.join("\n"))
                                    .collect()
                            };

                            for (part_id, content) in self.current_parts.drain(..) {
                                if part_id > 0 && part_id <= parts.len() {
                                    parts[part_id - 1] = content;
                                }
                            }

                            let new_content = parts.join("\n");

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
                    b"parts_to_edit" => {
                        if let Some(path) = self.current_path.take() {
                            let part_ids = self
                                .current_parts
                                .iter()
                                .map(|(id, _)| *id)
                                .collect::<Vec<usize>>();
                            println!("File: {}, Parts to edit: {:?}", path, part_ids);
                        }
                    }
                    b"preprocessor_prompt" => {
                        let prompt = self
                            .current_parts
                            .drain(..)
                            .map(|(_, content)| content)
                            .collect::<Vec<String>>()
                            .join("\n");
                        println!("Preprocessor Prompt: {}", prompt);
                    }
                    b"response" => {
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
                Ok(Event::Eof) => break,
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
