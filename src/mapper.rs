
//TODO: Make the Mapper struct into an interface to two iterators. Will need to wrap both, one an error iterator and the other a urlentry iterator, probably should include accessors for UrlEntry in this scope rather than have the user responsible for them.


extern crate xml;

use url::{ Url, ParseError };
use reqwest::{ Client, Error as ReqwestError };
use robotparser::RobotFileParser;
use sitemap::reader::{SiteMapReader, SiteMapEntity};
use sitemap::structs::{SiteMapEntry, UrlEntry};
use xml::reader::{Error as XmlError};

use util::to_base_url;

#[derive( Debug )]
pub enum MapperError {
    InvalidHost,
    NoDomain,
    ConnectionError( ReqwestError ),
    ParseError( ParseError ),
    XmlError( XmlError ),
}

struct PartialMapFetchError{
    errors: Vec<MapperError>,
    urls: Vec<UrlEntry>,
}

pub struct Mapper {
    sites:Vec<UrlEntry>,
    fetch_fails: Vec<MapperError>,
}

//Exposing the internal iterator must consume the Mapper, maybe allow as a convenience a way to go back

//This is going to take a while, look at parallelization options
impl Mapper {

    pub fn new( url:Url, client:Client ) -> Result< Mapper, MapperError > {
        let host = to_base_url( url )?;
        let robots_url = Mapper::guess_robots( &host )?;

        let sitemap_urls:Vec<Url> = match client.get( robots_url.clone( ) ).send( ) {
            Ok( mut r ) => {
                let t = RobotFileParser::new( robots_url );
                t.from_response( &mut r );
                t.get_sitemaps( "rs_pider" ) //TODO: The agent string needs to be configurable
            }
            Err( _e ) => {
                /* e is some form of connection error most probably
                I need to figure out how I want to deal with those and then fall back to guessing
                sitemap */
                vec![Mapper::guess_sitemap( &host )?]
            }
        };

        match Mapper::sitemap_descend( client, sitemap_urls, |u|{u} ) {
            Ok( urls ) => { Ok( Mapper{ sites:urls, fetch_fails:vec![] } ) }
            Err( partials ) => {
                Ok( Mapper{ sites:partials.urls, fetch_fails:partials.errors } )
            }
        }
    }

    fn guess_robots( url:&Url ) -> Result< Url, MapperError > {
        match url.join( "robots.txt" ) {
            Ok( u ) => return Ok( u ),
            Err( e ) => return Err( MapperError::ParseError( e ) ), /* in principle this never happens?
            We've already tested the error which would cause this in to_base_url() */
        }
    }
    fn guess_sitemap( url:&Url ) -> Result< Url, MapperError > {
        match url.join( "sitemap.xml" ){
            Ok( u ) => return Ok( u ),
            Err( e ) => return Err( MapperError::ParseError( e ) ),
        }
    }

    /* NOTE: Is maybe jank, there probably is a better way than using a function parameter here to
     * convert to a Url */
    fn sitemap_descend <T> ( client:Client, mut maps: Vec<T>, url_from:fn( T ) -> Url )
                        -> Result< Vec<UrlEntry>, PartialMapFetchError > {
        let mut ret = Vec::new( );
        let mut err_accum = Vec::new( );
        let mut submaps = Vec::new( );

        for map in maps.drain( .. ) {
            match client.get( url_from( map ) ).send( ) {
                Ok( mut r ) => {
                    //TODO: LIFETIMES
                    SiteMapReader::new( r.text( )
                                        .unwrap( )
                                        .as_bytes( ) ).
                        for_each( |event| {
                            match event {
                                SiteMapEntity::Url( u ) => ret.push( u ),
                                SiteMapEntity::SiteMap( s ) => submaps.push( s ),
                                SiteMapEntity::Err( e ) => err_accum.push( MapperError::XmlError( e ) )
                            }
                        } );
                }
                Err( e ) => {
                    err_accum.push( MapperError::ConnectionError( e ) );
                }
            }
        };

        if !submaps.is_empty( ) {
            match Mapper::sitemap_descend( client, submaps,
                                           |entry:SiteMapEntry|{ entry.loc.get_url().unwrap()} ) {
                Ok( urls ) => ret.extend( urls.iter( ).cloned( ) ),
                Err( mut partials ) => {
                    ret.extend( partials.urls.iter( ).cloned( ) );
                    partials.errors.drain( .. ).for_each( |e| { err_accum.push( e ) } );
                }
            };
        }

        if err_accum.is_empty( ) {
            Ok( ret )
        } else {
            Err( PartialMapFetchError{ urls: ret, errors:err_accum } )
        }
    }


}

