//! A stand-in for the one paid call that a picture needs, so that a test converts a picture with
//! no model. It answers every call with the same transcription of one figure, and counts the calls.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use ocr::content::PageBox;
use ocr::convert::reply::{TranscribedPage, TranscribedPiece};
use ocr::convert::services::{Answer, CallUsage, ImageServices, ServiceError};

pub const LABEL: &str = "Figure 24-12";
pub const CAPTION: &str = "FTSE 100 volatility surface. March 16, 2012.";
const EXPLANATION: &str = "A three-dimensional surface chart of the implied volatility of FTSE 100 options on March 16, 2012. \
The axis along the front is the exercise price, from 4000 to 8000. The axis that runs into the picture is the months to expiration, from 3 to 17. \
The vertical axis is the implied volatility, from 10% to 45%. The surface is highest at the lowest exercise prices, falls as the exercise price rises toward the level of the index, \
and a tall steep wall rises around an exercise price of 6000 for the longer expirations. The chart shows that implied volatility is not the same at every exercise price or at every expiration.";

#[derive(Default)]
pub struct StandInImageServices {
    calls: AtomicUsize,
}

impl StandInImageServices {
    /// How many times the model was asked.
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::Relaxed)
    }
}

impl ImageServices for StandInImageServices {
    async fn transcribe(
        &self,
        _picture: &Path,
        _correction: Option<&str>,
    ) -> Result<Answer<TranscribedPage>, ServiceError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        let figure = TranscribedPiece::Figure {
            number: 1,
            label: Some(LABEL.to_owned()),
            caption: Some(CAPTION.to_owned()),
            printed_text: vec![
                "Exercise price".to_owned(),
                "Months to expiration".to_owned(),
                "Implied volatility".to_owned(),
            ],
            bounds: PageBox {
                left: 0,
                top: 0,
                right: 1000,
                bottom: 1000,
            },
            explanation: EXPLANATION.to_owned(),
        };
        Ok(Answer {
            value: TranscribedPage {
                printed_page_number: None,
                running_header: None,
                pieces: vec![figure],
                discusses: Vec::new(),
                starts_mid_sentence: false,
                ends_mid_sentence: false,
            },
            usage: CallUsage {
                model: "stand-in".to_owned(),
                cost_usd: 0.0,
                output_tokens: 1,
                thinking_tokens: 0,
                seconds: 0.0,
            },
        })
    }
}
