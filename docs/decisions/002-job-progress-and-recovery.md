# ADR 002: Channel progress with registry recovery

Status: accepted

An anonymization job publishes status through a Tauri IPC channel. The frontend normally consumes that stream and starts status polling only after the channel is silent.

The channel is not authoritative. Job lifecycle state remains in the registry, and status commands support initial attachment, reconnection, missed terminal messages, and degraded-channel recovery. A channel send failure never changes job execution.
