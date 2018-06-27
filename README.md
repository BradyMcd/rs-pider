# rs-pider

The Rust ecosystem currently has a rich variety of high quality parsers afforded to it by packages like
serde. Both robots.txt parsing crates and sitemaps.xml parsers exist, the next logical step in building
a web spider in Rust is combining the two in order to take advantage of all the owner-supplied metadata
of a given site.

## Usage

Don't, yet.

## Function

In its current state, rs-pider is very much a work in process. Currently, it will take an arbitrary url
and attempt to fetch the host's robots.txt and sitemap.xml. Currently it does this all at once, so 
calling new( ) is a very slow process. Nothing is currently done to prettify or simplify the process of
getting useful urls out of the structure either, they are stored in a Vec<UrlEntry> reused from the
sitemap crate.
