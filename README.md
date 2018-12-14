# rs-pider

The Rust ecosystem currently has a rich variety of high quality parsers afforded to it by packages like
serde. Both robots.txt parsing crates and sitemaps.xml parsers exist, the next logical step in rolling
your own web spider in Rust is combining the two in order to take advantage of all the owner-supplied 
metadata of a given site.

## Usage

Don't, yet.

## Function

The current state of rs-pider is quite rough. It's a single structure, meta::SiteMeta, which handles
recursively fetching all sitemap.xml files discoverable on a given domain using the robots.txt file as
an entry point.

## Todo before cargo release

(a) Add filters both for sitemap.xml paths and url paths passed out (make a UrlFilter module)  
(z) Care about robots.txt exclusions  
(b) Think about how long-running spiders are going to reintegrate stale, already fetched or failed urls  
(b) Add conditional fetch based on HEAD hashes  
(c) Split off and generalize the segmented data structure for later use with the UrlEntry type  

Hes: (z) (c)  
Odi: (a) (b)  
