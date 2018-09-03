//

use std::collections::VecDeque;

use sitemap::structs::SiteMapEntry;
use sitemap::reader::SiteMapReader;
use robotparser::RobotFileParser;
use base_url::BaseUrl;
use url::Url;

use reqwest::{Client, Error as ReqwestError};

pub struct SiteMeta< 'a > {
    client: Client,
    //I think I need two barriers in this Deque, one for retry failures and one for timing
    known_maps: VecDeque< SiteMapEntry >,
    curr_map: Option< ( SiteMapReader< &'a [u8] >, SiteMapEntry ) >,
    robots: RobotFileParser< 'a >,
    base_url: BaseUrl,
}

fn guess_sitemap( url: &BaseUrl ) -> BaseUrl {
    let mut ret = url.clone( );
    ret.set_path( "sitemap.xml" );
    return ret;
}

impl< 'a > Iterator for SiteMeta< 'a > {
    type Item = BaseUrl;
    fn next( &mut self ) -> Option<Self::Item> {
        //maybe this is getting wrapped up in a loop?
        if self.curr_map.is_none( ) { // NOTE
            if self.known_maps.is_empty( ) {
                self.known_maps.push_back( Url::from( guess_sitemap( &self.base_url ) ).into( ) );
            }
            // at this point known_maps always has at least 1 entry.
            let next_entry = self.known_maps.pop_front( ).unwrap( );
            let response = self.client.get::<Url>( next_entry.loc.get_url( ).unwrap( ) ).send( );

            //r is a reqwest response. What can I do with *you*
            match response {
                Ok( r ) => {
                    self.curr_map = Some( ( SiteMapReader::new( r ), next_entry ) );
                }
                Err( e ) => {
                    //NOTE: Here be dragons
                }
            }
        }
        None
    }
}

fn guess_robots( url:&BaseUrl ) -> BaseUrl {
    let mut ret = url.clone( );
    ret.set_path( "robots.txt" );
    return ret;
}

impl< 'a > SiteMeta< 'a > {

    /// Builds a SiteMeta structure from a BaseUrl pointing to a robots.txt file on a server somewhere.
    ///
    /// # Errors:
    /// If an error occurs trying to fetch the robots.txt file it is returned instead. See
    /// [Link](https://docs.rs/reqwest/0.8.6/reqwest/struct.Error.html) for more
    pub fn from_robots_url( robots_url:&BaseUrl ) -> Result< ( SiteMeta< 'a > ), ReqwestError > {
        let client = Client::new( ); //TODO: add client setup
        let response = client.get::< Url >( robots_url.clone( ).into( ) ).send( );
        let robots_txt:RobotFileParser< 'a > = RobotFileParser::< 'a >::from( robots_url.clone( ) );
        let known_maps:VecDeque< SiteMapEntry >;
        let host = robots_url.clone( );
        host.set_path( "/" );

        match response{
            Ok( mut r ) => {
                robots_txt.from_response( &mut r );
            }
            Err( e ) => {
                return Err( e );
            }
        };

        known_maps = robots_txt.get_sitemaps( "useragent" )
            .into_iter( )
            .map( |u:Url|{ u.into( ) } )
            .collect( );

        Ok( SiteMeta {
            client: client,
            known_maps: known_maps,
            curr_map: None,
            robots: robots_txt,
            base_url: host,
        } )
    }

    /// Builds a SiteMeta structure from a BaseUrl pointing to some host. It will automatically guess
    /// that robots.txt is stored in the standard location, at the root of the host.
    ///
    /// # Errors:
    /// If an error occurs trying to fetch the robots.txt file it is returned instead. See
    /// [Link](https://docs.rs/reqwest/0.8.6/reqwest/struct.Error.html) for more
    pub fn from_url( url:&BaseUrl ) -> Result< ( SiteMeta< 'a > ), ReqwestError > {
        Self::from_robots_url( &guess_robots( url ) )
    }

}

// TODO: Implement Iterator next, that way I can actually get BaseUrl streams out

