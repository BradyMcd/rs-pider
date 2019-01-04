//

use rs_pider_robots::RobotsParser;

use base_url::BaseUrl;
use base_url::TryFrom;

use sitemap::structs::{ SiteMapEntry, Location };
use sitemap::reader::{ SiteMapReader, SiteMapEntity };
use url::Url;

use reqwest::{Client, Response, Error as ReqwestError};
use reqwest::header::{ ETAG, LAST_MODIFIED };
use reqwest::Method;

use std::hash::{ Hash, Hasher };
use sectioned_list::SectionedList;

fn guess_robots( url:&BaseUrl ) -> BaseUrl {
    let mut ret = url.clone( );
    ret.set_path( "robots.txt" );
    return ret;
}

enum ResID {
    ETag( String ),
    LastMod( String ),
    Empty,
}

impl ResID {

    fn from_response( res: &Response ) -> Self {
        if res.status( ).is_success( ) {
            match res.headers( ).get( ETAG ) {
                Some( et ) => return ResID::ETag( et.to_str( ).unwrap( ).to_string( ) ),
                _ => { /* DO NOTHING */ }
            }
            match res.headers( ).get( LAST_MODIFIED ) {
                Some( s ) => {
                    return ResID::LastMod( s.to_str( ).unwrap( ).to_string( ) )
                }
                _ => { /* DO NOTHING */ }
            }
        }

        ResID::empty( )
    }

    fn empty( ) -> Self {
        ResID::Empty
    }

    fn is_empty( &self ) -> bool {
        match self {
            ResID::Empty => true,
            _ => false,
        }
    }

    fn keytxt( &self ) -> &str {
        assert!( !self.is_empty( ) );
        match self {
            ResID::LastMod( _ ) => "If-Modified-Since",
            ResID::ETag( _ ) => "If-Match",
            ResID::Empty => "",
        }
    }

    fn valtxt( &self ) -> &str {
        assert!( !self.is_empty( ) );
        match self {
            ResID::LastMod( s ) => s.as_str( ),
            ResID::ETag( s ) => s.as_str( ),
            _ => "",
        }
    }
}

struct KnownSitemap {
    location: BaseUrl,
    last_id: ResID,
}

impl From< BaseUrl > for KnownSitemap {

    fn from( url: BaseUrl ) -> Self {
        KnownSitemap{
            location: url,
            last_id: ResID::empty( ),
        }
    }
}

impl TryFrom< SiteMapEntry > for KnownSitemap {
    type Err = ();
    fn try_from( entry: SiteMapEntry ) -> Result< Self, Self::Err > {
        let location = match entry.loc.get_url( ) {
            Some( url ) => BaseUrl::from( url ), //PANIC! maybe.
            None => return Err(( ))
        };

        Ok( KnownSitemap {
            location: location,
            last_id: ResID::empty( ),
        } )
    }
}

impl PartialEq for KnownSitemap {
    fn eq( &self, rhs: &Self ) -> bool {
        self.location == rhs.location
    }
}

impl Hash for KnownSitemap {

    fn hash< H: Hasher > ( &self, hasher: &mut H ) {
        self.location.path( ).hash( hasher );
    }
}

impl KnownSitemap {

    fn fetch_map( &mut self, client: &Client ) -> Result< Response, ReqwestError > {
        let response: Result< Response, ReqwestError >;
        let fetch_url = Url::from( self.location.clone( ) );
        let mut request = client.request( Method::GET, fetch_url );

        if !self.last_id.is_empty( ) {
            request = request.header( self.last_id.keytxt( ), self.last_id.valtxt( ) );
        }

        response = request.send( );
        match &response {
            Ok( res ) =>{
                self.last_id = ResID::from_response( res );
            }
            Err( _ ) => { /*DO NOTHING*/ }
        }
        return response;
    }
}

/*
 * Public
 */
pub struct SiteMeta {
   client: Client,
    known_maps: SectionedList< KnownSitemap >,
    curr_map: Option< ( SiteMapReader< Response > ) >,
    robots: RobotsParser,
    base_url: BaseUrl,
}

fn guess_sitemap( url: &BaseUrl ) -> BaseUrl {
    let mut ret = url.clone( );
    ret.set_path( "sitemap.xml" );
    return ret;
}

impl SiteMeta {

    /// Only called internally, ```populate_known()``` first checks if there are any known maps. If
    /// there are none it makes the idiomatic guess that a sitemap.xml is placed at the root of the
    /// domain.
    /// It doesn't then do the following until the TODO is cleared:
    /// Otherwise it checks if there are only entries in the stale section of the maps list, if that's
    /// the case it uses the stale map urls to populate the queue. In future this process will be
    /// filtered and the function may still return ```false``` indicating failure to populate the
    /// ```known_maps``` field.
    //TODO: Add a filtered_merge_stale( ) in this function
    #[inline]
    fn populate_known( &mut self ) -> bool {

        if self.known_maps.is_empty( ) {

            self.known_maps.push_main (
                guess_sitemap( &self.base_url ).into( )
            );
        // }else if self.known_maps.is_stale( ) {

        //     //I should use the filtered version instead, but how should I filter?
        //     self.known_maps.merge_stale( );
        //     if self.known_maps.is_stale( ) { return false; }
        }
        true
    }

    /// Only called internally, ```fetch_next( )``` takes the next entry in the ```known_maps``` queue
    /// if one exists a GET request is made for the indicated url. The response to this request is then
    /// analyzed. On a successful data return a ```SiteMapReader``` is built out of the response text
    /// and stored in the ```curr_map``` slot. In the case of an error the map is pushed into the stale
    /// queue of ```known_maps``` and currently no further action is taken
    #[inline]
    fn fetch_next_map( &mut self ) -> bool {
        {
            let next_entry = self.known_maps.peek_main( );
            if { next_entry.is_some( ) } {
                let response = next_entry.unwrap( ).fetch_map( &self.client );

                match response {
                    Ok( r ) => {
                        self.curr_map = Some( SiteMapReader::< Response >::new( r ) );
                        return true;
                    }
                    Err( _e ) => {
                        //TODO: Here be dragons
                    }
                }
            }
        }
        self.known_maps.skip_main( );
        false
    }

    /// Uses a combination of guesswork and collected metadata to populate the ```curr_map``` slot of
    /// this metadata structure. If the ```curr_map``` is already populated no action is taken. If no
    /// maps are listed a guess as to a map location is made: in a file named "sitemap.xml" at the root
    /// of the domain. If all ```known_maps``` are put into the stale section of the queue in the
    /// process of attempting to populate the ```curr_map``` slot, ```false``` is returned to indicate
    /// an error.
    fn populate_current( &mut self ) -> bool {
        if self.curr_map.is_some( ){
            /* nothing */
        } else if !self.populate_known( ) {
            return false;
        } else {
            while !self.fetch_next_map( ) {
                if self.known_maps.is_alt( ) { return false; }
            }
        }
        true
    }

    //TODO: .expect( ) is brittle
    //TODO: sitemap.xml CAN contain relative path URLs
    //TODO: 
    fn next_in_map( &mut self ) -> Option< BaseUrl > {

        let mut curr_map = self.curr_map.take( ).unwrap( );
        let mut w_entry = curr_map.next( );

        while w_entry.is_some( ) {
            let entry = w_entry.unwrap( );
            match entry {

                SiteMapEntity::Url( ue ) => {
                    match ue.loc {
                        Location::Url( u ) => {
                            self.curr_map.get_or_insert( curr_map );
                            return Some( BaseUrl::try_from( u )
                                         .expect( "Sitemap contains an invalid url" ) );
                        }
                        _ => { /*Do Nothing*/ }
                    }
                }

                SiteMapEntity::SiteMap( se ) => {
                    match se.loc {
                        Location::Url( u ) => {
                            self.known_maps.push_main( BaseUrl::try_from( u ).expect(
                                "Sitemap contains an invalid url" ).into( ) );
                        }
                        _ => { /*Do Nothing*/}
                    }
                }
                _ => { /*Do Nothing*/ }
            }
            w_entry = curr_map.next( );
        }

        self.known_maps.skip_main();
        None
    }
}

impl Iterator for SiteMeta {
    type Item = BaseUrl;
    fn next( &mut self ) -> Option< Self::Item > {
        let mut ret: Option< Self::Item >;

        loop {
            if !self.populate_current( ) {
                return None;
            }

            ret = self.next_in_map( );
            if ret.is_some( ) {
                return ret;
            }
        }
    }
}

impl SiteMeta {

    /// Builds a SiteMeta structure from a BaseUrl pointing to a robots.txt file on a server somewhere.
    ///
    /// # Errors:
    /// If an error occurs trying to fetch the robots.txt file it is returned instead. See
    /// [Link](https://docs.rs/reqwest/0.8.6/reqwest/struct.Error.html) for more.
    pub fn from_robots_url( robots_url:&BaseUrl ) -> Result< ( SiteMeta ), ReqwestError > {
        let client = Client::new( ); //TODO: add client setup
        let response = client.get::< Url >( robots_url.clone( ).into( ) ).send( );
        let robots_txt;
        let known_maps: SectionedList< KnownSitemap >;
        let mut host = robots_url.clone( );
        host.set_path( "/" );

        match response{
            Ok( r ) => {
                robots_txt = RobotsParser::from_response( r );
            }
            Err( e ) => {
                return Err( e );
            }
        };

        known_maps = robots_txt.get_sitemaps( ).into_iter( ).collect( );

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
    pub fn from_url( url:&BaseUrl ) -> Result< ( SiteMeta ), ReqwestError > {
        Self::from_robots_url( &guess_robots( url ) )
    }

}

