use crate::{
    Perspective, errors::SocialContextResult
};

pub fn render() -> SocialContextResult<Perspective> {
    Ok(Perspective {
        links: vec![]
    })
}