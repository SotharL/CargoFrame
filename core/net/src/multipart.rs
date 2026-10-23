//! `multipart/form-data` (and `multipart/related`, which Google Drive uploads use) bodies.

enum Part {
    Field {
        name: String,
        value: String,
    },
    File {
        name: String,
        file_name: String,
        content_type: String,
        data: Vec<u8>,
    },
    /// A part with only a content type (multipart/related).
    Raw {
        content_type: String,
        data: Vec<u8>,
    },
}

pub struct Multipart {
    boundary: String,
    related: bool,
    parts: Vec<Part>,
}

/// Header values must not carry line breaks or quotes, or a file name could end a header early.
fn header_safe(s: &str) -> String {
    s.chars()
        .filter(|c| *c != '\r' && *c != '\n')
        .map(|c| if c == '"' { '\'' } else { c })
        .collect()
}

fn random_boundary() -> String {
    let mut bytes = [0u8; 12];
    if getrandom::fill(&mut bytes).is_err() {
        // Extremely unlikely; the time still gives a boundary that differs between requests.
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        bytes.copy_from_slice(&t.to_le_bytes()[..12]);
    }
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("cargoframe-{hex}")
}

impl Multipart {
    pub fn form_data() -> Self {
        Self {
            boundary: random_boundary(),
            related: false,
            parts: Vec::new(),
        }
    }

    pub fn related() -> Self {
        Self {
            boundary: random_boundary(),
            related: true,
            parts: Vec::new(),
        }
    }

    /// For tests that compare bodies exactly.
    pub fn with_boundary(mut self, boundary: &str) -> Self {
        self.boundary = boundary.to_string();
        self
    }

    pub fn field(mut self, name: &str, value: &str) -> Self {
        self.parts.push(Part::Field {
            name: name.into(),
            value: value.into(),
        });
        self
    }

    pub fn file(mut self, name: &str, file_name: &str, content_type: &str, data: Vec<u8>) -> Self {
        self.parts.push(Part::File {
            name: name.into(),
            file_name: file_name.into(),
            content_type: content_type.into(),
            data,
        });
        self
    }

    pub fn raw(mut self, content_type: &str, data: Vec<u8>) -> Self {
        self.parts.push(Part::Raw {
            content_type: content_type.into(),
            data,
        });
        self
    }

    pub fn content_type(&self) -> String {
        let kind = if self.related { "related" } else { "form-data" };
        format!("multipart/{kind}; boundary={}", self.boundary)
    }

    /// Make sure the boundary appears in no part's data.
    fn settle_boundary(&mut self) {
        for _ in 0..8 {
            let marker = format!("--{}", self.boundary).into_bytes();
            let clash = self.parts.iter().any(|p| {
                let data: &[u8] = match p {
                    Part::Field { value, .. } => value.as_bytes(),
                    Part::File { data, .. } | Part::Raw { data, .. } => data,
                };
                data.windows(marker.len()).any(|w| w == marker.as_slice())
            });
            if !clash {
                return;
            }
            self.boundary = random_boundary();
        }
    }

    /// The content type header and the body, with a boundary that is certain not to clash.
    /// Always use this (not [`Multipart::content_type`] then [`Multipart::into_bytes`]) when
    /// sending, because settling the boundary can change it.
    pub fn finish(mut self) -> (String, Vec<u8>) {
        self.settle_boundary();
        (self.content_type(), self.into_bytes())
    }

    pub fn into_bytes(self) -> Vec<u8> {
        let mut out = Vec::new();
        for part in &self.parts {
            out.extend_from_slice(format!("--{}\r\n", self.boundary).as_bytes());
            match part {
                Part::Field { name, value } => {
                    out.extend_from_slice(
                        format!(
                            "Content-Disposition: form-data; name=\"{}\"\r\n\r\n",
                            header_safe(name)
                        )
                        .as_bytes(),
                    );
                    out.extend_from_slice(value.as_bytes());
                }
                Part::File {
                    name,
                    file_name,
                    content_type,
                    data,
                } => {
                    out.extend_from_slice(
                        format!(
                            "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\nContent-Type: {}\r\n\r\n",
                            header_safe(name),
                            header_safe(file_name),
                            header_safe(content_type)
                        )
                        .as_bytes(),
                    );
                    out.extend_from_slice(data);
                }
                Part::Raw { content_type, data } => {
                    out.extend_from_slice(
                        format!("Content-Type: {}\r\n\r\n", header_safe(content_type)).as_bytes(),
                    );
                    out.extend_from_slice(data);
                }
            }
            out.extend_from_slice(b"\r\n");
        }
        out.extend_from_slice(format!("--{}--\r\n", self.boundary).as_bytes());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_data_is_laid_out_as_the_standard_says() {
        let body = Multipart::form_data()
            .with_boundary("B")
            .field("title", "Hi")
            .file("file", "a.png", "image/png", vec![1, 2, 3])
            .into_bytes();
        let want = b"--B\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\nHi\r\n\
--B\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.png\"\r\nContent-Type: image/png\r\n\r\n\x01\x02\x03\r\n\
--B--\r\n";
        assert_eq!(
            String::from_utf8_lossy(&body),
            String::from_utf8_lossy(want)
        );
    }

    #[test]
    fn related_parts_carry_only_a_content_type() {
        let m = Multipart::related()
            .with_boundary("R")
            .raw("application/json", b"{}".to_vec());
        assert_eq!(m.content_type(), "multipart/related; boundary=R");
        let body = m.into_bytes();
        assert_eq!(
            String::from_utf8_lossy(&body),
            "--R\r\nContent-Type: application/json\r\n\r\n{}\r\n--R--\r\n"
        );
    }

    #[test]
    fn names_cannot_break_out_of_their_header() {
        let body = Multipart::form_data()
            .with_boundary("B")
            .file("f\"x", "evil\"\r\nX-Injected: 1.png", "image/png", vec![])
            .into_bytes();
        let text = String::from_utf8_lossy(&body);
        assert!(!text.contains("\r\nX-Injected"));
        assert!(text.contains("filename=\"evil'X-Injected: 1.png\""));
        assert!(text.contains("name=\"f'x\""));
    }

    #[test]
    fn a_boundary_that_appears_in_the_data_is_replaced_and_the_header_follows() {
        let m = Multipart::form_data().with_boundary("CLASH").file(
            "f",
            "a.bin",
            "application/octet-stream",
            b"xx--CLASHxx".to_vec(),
        );
        let (ct, body) = m.finish();
        assert!(!ct.contains("CLASH"), "{ct}");
        let boundary = ct.split("boundary=").nth(1).unwrap();
        let text = String::from_utf8_lossy(&body);
        assert!(text.starts_with(&format!("--{boundary}\r\n")), "{text}");
        assert!(
            text.trim_end().ends_with(&format!("--{boundary}--")),
            "{text}"
        );
    }

    #[test]
    fn boundaries_differ_between_bodies() {
        assert_ne!(
            Multipart::form_data().content_type(),
            Multipart::form_data().content_type()
        );
    }
}
