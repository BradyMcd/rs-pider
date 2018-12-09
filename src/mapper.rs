//
//TODO: Make the Mapper struct into an interface to two iterators. Will need to wrap both, one an error iterator and the other a urlentry iterator, probably should include accessors for UrlEntry in this scope rather than have the user responsible for them.

//NOTE: Maybe I want to make this into what is essentially a 2 channel system, an error channel and the main iterators

extern crate xml;

use std::collections::LinkedList;

use url::{ Url, ParseError };
use reqwest::{ Client, Error as ReqwestError };
use robotparser::RobotFileParser;
use sitemap::reader::{SiteMapReader, SiteMapEntity};
pub use sitemap::structs::{SiteMapEntry, UrlEntry};
use xml::reader::{Error as XmlError};

use util::{base_url, is_host_only};

#[derive( Debug )]
pub enum MapperError {
    InvalidHost,
    NoDomain,
    ConnectionError( ReqwestError ),
    ParseError( ParseError ),
    XmlError( XmlError ),

    ///The ImpossibleError should never be thrown, if you ever see it it's an indication that either
    ///the libraries I'm using are throwing undocumented errors around or the environment isn't sane
    ImpossibleError,
}

struct PartialMapFetchError{
    errors: LinkedList<MapperError>,
    urls: LinkedList<UrlEntry>,
}

///Represents all information made available to web crawlers under a single domain based on robots.txt
///and Sitemap.xml standards
pub struct Mapper<'a> {
    sites: LinkedList<UrlEntry>,
    fetch_fails: LinkedList<MapperError>,
    known_maps: LinkedList< SiteMapEntry >,
    host: Url,
    robots: RobotFileParser<'a>,
    seed_url: Option<Url>,
}


/****************
 * Private
 */

impl From<ReqwestError> for MapperError{
    fn from( e:ReqwestError ) -> MapperError {
        MapperError::ConnectionError( e )
    }
}

impl From<ParseError> for MapperError {
    fn from( e:ParseError ) -> MapperError {
        MapperError::ParseError( e )
    }
}

impl From<XmlError> for MapperError {
    fn from( e:XmlError ) -> MapperError {
        MapperError::XmlError( e )
    }
}

fn guess_robots( url:&Url ) -> Result< Url, MapperError > {
    match url.join( "robots.txt" ) {
        Ok( u ) => return Ok( u ),
        Err( e ) => return Err( MapperError::ParseError( e ) ),
    }
}
fn guess_sitemaps( url:&Url ) -> Result< LinkedList<Url>, MapperError > {
    let host = base_url( url )?;
    let mut ret = LinkedList::new( );
    ret.push_back( host.clone( ).join( "sitemap.xml" )? );

    for seg in url.path_segments( ).unwrap( ) {
        ret.push_back( host.clone( ).join( seg )?
                  .join( "sitemap.xml" )? );
    }

    Ok( ret )
}

impl Mapper {

    pub fn new( host:Url ) -> Result< Mapper, MapperError > {

        if is_host_only( &host )? {
            let robots = guess_robots( &host )?;
            Ok( Mapper {
                sites: LinkedList::new( ),
                fetch_fails: LinkedList::new( ),
                known_maps: LinkedList::new( ),
                host: host,
                robots: RobotFileParser::new( robots ),
                seed_url: None,
            } )
        } else {

            let mut maps = LinkedList::new();
            let _host;
            let robots;
            let seed = Some( host.join( "/" )? );

            if host.path( ).contains( "robots.txt" ) {
                _host = base_url( &host )?;
                robots = host;
            } else if host.path( ).contains( "sitemap.xml" ) {
                _host = base_url( &host )?;
                robots = guess_robots( &host )?;
                maps.push_back( host.into( ) );
            } else{
                _host = base_url( &host )?;
                robots = guess_robots( &host )?;
            }
            let robot_parser = RobotFileParser::new( robots );

            Ok( Mapper {
                sites: LinkedList::new( ),
                fetch_fails: LinkedList::new( ),
                known_maps: maps,
                host: _host,
                robots: robot_parser,
                seed_url: seed,
            } )

        }
    }

    pub fn fetch_robots( mut self ) {
    }

    pub fn guess_sitemap_urls( &mut self ) {

        self.known_maps.append( guess_sitemaps(
            &match self.seed_url {
                Some( url ) => url,
                None => self.host
            }
        ) );
    }



    /* NOTE: Is maybe jank, there probably is a better way than using a function parameter here to
     * convert to a Url */
    fn sitemap_descend <T> ( client:Client, maps: LinkedList<T>, url_from:fn( T ) -> Url )
                        -> Result< LinkedList<UrlEntry>, PartialMapFetchError > {
        let mut ret = LinkedList::new( );
        let mut err_accum = LinkedList::new( );
        let mut submaps = LinkedList::new( );

        for map in maps {
            match client.get( url_from( map ) ).send( ) {
                Ok( mut r ) => {
                    SiteMapReader::new( r.text( )
                                        .unwrap( )
                                        .as_bytes( ) ).
                        for_each( |event| {
                            match event {
                                SiteMapEntity::Url( u ) => ret.push_back( u ),
                                SiteMapEntity::SiteMap( s ) => submaps.push_back( s ),
                                SiteMapEntity::Err( e ) => err_accum.push_back( MapperError::XmlError( e ) )
                            }
                        } );
                }
                Err( e ) => {
                    err_accum.push_back( MapperError::ConnectionError( e ) );
                }
            }
        };

        if !submaps.is_empty( ) {
            match Mapper::sitemap_descend( client, submaps,
                                           |entry:SiteMapEntry|{ entry.loc.get_url().unwrap()} ) {
                Ok( urls ) => ret.extend( urls.iter( ).cloned( ) ),
                Err( mut partials ) => {
                    ret.extend( partials.urls.iter( ).cloned( ) );
                    err_accum.append( &mut partials.errors )
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
