//! Managed Chrome/Chromium with CDP, profile isolation per browser ID.
//! Used by the browser tool for screenshots, snapshots, and page interaction.
//!
//! # Features
//!
//! - **navigate**: Navigate to URLs and wait for page load
//! - **screenshot**: Capture page screenshots (full page or viewport)
//! - **snapshot**: Extract DOM with numbered element references
//! - **click**: Click elements by reference number
//! - **type**: Type text into input fields
//! - **scroll**: Scroll the page or specific elements
//! - **evaluate**: Execute JavaScript in page context
//! - **wait**: Wait for elements to appear
//!
//! # Example
//!
//! ```ignore
//! use moltis_browser::{BrowserManager, BrowserConfig, BrowserRequest, BrowserAction};
//!
//! let config = BrowserConfig { enabled: true, ..Default::default() };
//! let manager = BrowserManager::new(config);
//!
//! let request = BrowserRequest {
//!     session_id: None,
//!     action: BrowserAction::Navigate { url: "https://example.com".into() },
//!     timeout_ms: 30000,
//!     sandbox: None,
//!     browser: None,
//!     chat_session_key: None,
//! };
//!
//! let response = manager.handle_request(request).await;
//! ```

pub mod container;
pub mod detect;
pub mod error;
pub mod launch_target;
pub mod manager;
pub mod pool;
pub mod snapshot;
pub mod submit_gate;
pub mod types;

mod browserless;

pub use {
    error::Error,
    manager::BrowserManager,
    submit_gate::{authorize_submit, note_user_message},
    types::{
        BrowserAction, BrowserConfig, BrowserKind, BrowserPreference, BrowserRequest,
        BrowserResponse,
    },
};
