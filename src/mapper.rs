
use std::io::Read;

use url::{ Url, ParseError };
use reqwest::{ Client, Error as ReqwestError };
use robotparser::RobotFileParser;
use sitemap::reader::{SiteMapReader, SiteMapEntity};
use sitemap::structs::{SiteMapEntry, UrlEntry};

#[derive( Debug )]
enum MapperError {
    InvalidHost,
    NoDomain,
    PartialMapFetchError( ( Vec<ReqwestError>, Vec<UrlEntry> ) ),
    ConnectionError( ReqwestError ),
    ParseError( ParseError ),
}


struct Mapper {
    pub sites:Vec<UrlEntry>,
    fetch_fails: Vec<ReqwestError>,
}

//Exposing the internal iterator must consume the Mapper, maybe allow as a convenience a way to go back

//This is going to take a while, look at parallelization options
impl Mapper {

    ///takes a url and ensures it is in the form https://foo.bar/ and is a valid host
    /*TODO: since we may be getting a url from an untrusted source sanitizing it is potentially
     * important in a number of scenarios, it should probably be moved to a util module if and when it
     * becomes useful. */
    fn to_base_url( url:Url ) -> Result< Url, MapperError >{
        if url.cannot_be_a_base( ) {
            return Err( MapperError::InvalidHost );
        }
        url.set_scheme( "https" );
        url.set_path( "" );
        url.set_port( None );
        url.set_query( None );
        url.set_fragment( None );
        url.set_username( "" );
        url.set_password( None );
        Ok( url )
    }

    fn guess_robots( url:Url ) -> Result< Url, MapperError > {
        match url.join( "robots.txt" ) {
            Ok( u ) => return Ok( u ),
            Err( e ) => return Err( MapperError::ParseError( e ) ), /* in principle this never happens?
            We've already tested the error which would cause this in to_base_url() */
        }
    }
    fn guess_sitemap( url:Url ) -> Result< Url, MapperError > {
        match url.join( "sitemap.xml" ){
            Ok( u ) => return Ok( u ),
            Err( e ) => return Err( MapperError::ParseError( e ) ),
        }
    }

    /* NOTE: Is maybe jank, there probably is a better way than using a function parameter here to
     * convert to a Url */
    fn sitemap_descend <T> ( client:Client, maps:Vec<T>, url_from:fn( T ) -> Url )
                        -> Result< Vec<UrlEntry>, MapperError > {
        let ret = Vec::new( );
        let err_accum = Vec::new( );
        let submaps = Vec::new( );

        for map in maps.iter( ) {
            match client.get( url_from( *map ) ).send( ) {
                Ok( r ) => {
                    let tokens = SiteMapReader::new( r.text( )
                                                     .unwrap( )
                                                     .as_bytes( ) );
                    for event in tokens {
                        match event {
                            SiteMapEntity::Url( u ) => ret.push( u ),
                            SiteMapEntity::SiteMap( s ) => submaps.push( s ),
                        }
                    }
                }
                Err( e ) => {
                    err_accum.push( e );
                }
            }
        };

        if !submaps.is_empty( ) {
            match Mapper::sitemap_descend( client, submaps,
                                           |entry:SiteMapEntry|{ entry.loc.get_url().unwrap()} ) {
                Ok( urls ) => ret.extend( urls.iter( ).cloned( ) ),
                Err( MapperError::PartialMapFetchError( inner ) ) => {
                    let ( errs, urls ) = inner;
                    ret.extend( urls.iter( ).cloned( ) );
                    errs.iter( ).for_each( |e| { err_accum.push( *e ) } );
                }
            };
        }

        if err_accum.is_empty( ) {
            Ok( ret )
        } else {
            Err( MapperError::PartialMapFetchError( ( err_accum, ret ) ) )
        }
    }

    fn new( url:Url, client:Client ) -> Result< Mapper, MapperError > {
        let host = Mapper::to_base_url( url )?;
        let robots_url = Mapper::guess_robots( host )?;

        let mut sitemap_urls:Vec<Url> = match client.get( robots_url ).send( ) {
            Ok( r ) => {
                let t = RobotFileParser::new( robots_url );
                t.from_response( &mut r );
                t.get_sitemaps( "rs_pider" ) //TODO: The agent string needs to be configurable
            }
            Err( _e ) => {
                /* e is some form of connection error most probably
                I need to figure out how I want to deal with those and then fall back to guessing
                sitemap */
                vec![Mapper::guess_sitemap( host )?]
            }
        };

        match Mapper::sitemap_descend( client, sitemap_urls, |u|{u} ) {
            Ok( urls ) => { Ok( Mapper{ sites:urls, fetch_fails:vec![] } ) }
            Err( MapperError::PartialMapFetchError(inner) ) => {
                let (errs, urls) = inner;
                Ok( Mapper{ sites:urls, fetch_fails:errs } )
            }
        }
    }
}
//TODO:Nonexhaustive patterns!
