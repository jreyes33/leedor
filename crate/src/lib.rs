mod epub;
mod error;
mod utils;
mod web;
mod xml;

use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "wee_alloc")] {
        #[global_allocator]
        static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;
    }
}

pub use web::run;
