"""Private admission/checkpoint timings; no credentials or gameplay changes."""
from pathlib import Path
p=Path('crates/launcher/src/host/admission.rs');s=p.read_text()
needle='    ) -> Result<(), AdmissionError> {\n        if connection.handle_transfer_message'
assert s.count(needle)==1
s=s.replace(needle,'    ) -> Result<(), AdmissionError> {\n        let kind = match &message {\n            ClientMessage::InspectSession { .. } => Some("inspect"),\n            ClientMessage::Hello { resume, .. } => Some(if resume.is_some() { "hello-resume" } else { "hello-new" }),\n            ClientMessage::WelcomeChunkAck { .. } => Some("welcome-ack"),\n            ClientMessage::SaveBeforeLeave { .. } => Some("save-before-leave"),\n            _ => None,\n        };\n        if let Some(kind) = kind {\n            eprintln!("HOST_ADMISSION_TRACE {}", serde_json::json!({\n                "at": crate::host::source_wall_clock_ms(), "kind": kind, "peer": connection.peer.id(),\n            }));\n        }\n        if connection.handle_transfer_message')
needle='        connection.welcome_revision = Some(self.metadata.revision);\n'
assert s.count(needle)==1
s=s.replace(needle,needle+'        eprintln!("HOST_ADMISSION_TRACE {}", serde_json::json!({\n            "at": crate::host::source_wall_clock_ms(), "kind": "admitted-awaiting-checkpoint",\n            "peer": connection.peer.id(), "revision": self.metadata.revision,\n        }));\n')
p.write_text(s)
p=Path('crates/launcher/src/host/authority.rs');s=p.read_text()
needle='        self.checkpoint_writer.submit(session.checkpoint(), self.metadata.clone())\n'
assert s.count(needle)==1
s=s.replace(needle,'        eprintln!("HOST_ADMISSION_TRACE {}", serde_json::json!({\n            "at": crate::host::source_wall_clock_ms(), "kind": "checkpoint-submit",\n            "revision": self.metadata.revision,\n        }));\n'+needle)
needle='        completion.result?;\n'
assert s.count(needle)==1
s=s.replace(needle,needle+'        eprintln!("HOST_ADMISSION_TRACE {}", serde_json::json!({\n            "at": crate::host::source_wall_clock_ms(), "kind": "checkpoint-complete",\n            "revision": completion.revision,\n        }));\n')
needle='            let payload =\n                self.prepare_completed_frame(Some(id), sequence, now_ms, None, |frame| {\n'
assert s.count(needle)==1
s=s.replace(needle,'            eprintln!("HOST_ADMISSION_TRACE {}", serde_json::json!({\n                "at": crate::host::source_wall_clock_ms(), "kind": "welcome-encode", "peer": peer_id,\n            }));\n'+needle)
needle='            send_welcome(&connection.peer, payload)?;\n'
assert s.count(needle)==1
s=s.replace(needle,'            eprintln!("HOST_ADMISSION_TRACE {}", serde_json::json!({\n                "at": crate::host::source_wall_clock_ms(), "kind": "welcome-send", "peer": peer_id,\n                "bytes": payload.len(),\n            }));\n'+needle)
p.write_text(s)
