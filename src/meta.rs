//
// :81 :83

use std::collections::VecDeque;
use std::iter::FromIterator;

use try_from::TryFrom;

use sitemap::structs::{ SiteMapEntry, Location };
use sitemap::reader::{ SiteMapReader, SiteMapEntity };
use robotparser::RobotFileParser;
use base_url::BaseUrl;
use url::Url;

use reqwest::{Client, Response, Error as ReqwestError};

/*
 * Internals
 */

/// A concordantly filtered collection of SiteMap information with a fresh and stale pile
struct MapsList {
    new: VecDeque< SiteMapEntry >,
    stale: VecDeque< SiteMapEntry >,
}


//TODO: Since this is two collections having a full new section and even one element in the stale section for example could make bad things happen.
impl MapsList {

    fn new(  ) -> MapsList {
        MapsList{
            new: VecDeque::new(),
            stale: VecDeque::new(),
        }
    }

    fn is_empty( &self ) -> bool {
        self.new.is_empty( ) && self.stale.is_empty( )
    }

    fn is_stale( &self ) -> bool {
        self.new.is_empty( ) && !self.stale.is_empty( )
    }

    fn push_new( &mut self, data: SiteMapEntry ) {
        if !self.new.iter( ).any( | e |{ e.loc.get_url( ) == data. loc.get_url( ) } ) &&
            !self.stale.iter( ).any( | e |{ e.loc.get_url( ) == data.loc.get_url( ) } ) {
            self.new.push_back( data );
        }
    }

    fn push_stale( &mut self, data: SiteMapEntry ) {
        self.stale.push_back( data );
    }

    fn pop( &mut self ) -> Option< SiteMapEntry > {
        self.new.pop_front( )
    }

    //TODO: Split this
    fn len( &self ) -> usize {
        self.new.len( )
    }

    fn merge_stale( &mut self ) {
        self.new.append( &mut self.stale );
    }

    fn filter_merge_stale( &mut self, filter:&mut FnMut( &SiteMapEntry )->bool ) {
        let ( n, s ): ( VecDeque< SiteMapEntry >, VecDeque< SiteMapEntry > ) =
            self.stale.drain( .. ).partition( |entry|{ filter( entry ) } );

        self.new.extend( n );
        self.stale.extend( s );
    }
}
impl< T: Into< SiteMapEntry > > FromIterator< T > for MapsList {
    fn from_iter< I:IntoIterator< Item=T > >( iter:I ) -> Self {
        let mut ret = MapsList::new( );

        for item in iter {
            ret.push_new( item.into( ) );
        }
        return ret;
    }
}

fn guess_robots( url:&BaseUrl ) -> BaseUrl {
    let mut ret = url.clone( );
    ret.set_path( "robots.txt" );
    return ret;
}

/*
 * Public
 */

pub struct SiteMeta< 'r > {
    client: Client,
    //I think I need two barriers in this Deque, one for retry failures and one for timing
    known_maps: MapsList,
    curr_map: Option< ( SiteMapReader< Response >, SiteMapEntry ) >,
    robots: RobotFileParser< 'r >,
    base_url: BaseUrl,
}

fn guess_sitemap( url: &BaseUrl ) -> BaseUrl {
    let mut ret = url.clone( );
    ret.set_path( "sitemap.xml" );
    return ret;
}

impl< 'r > SiteMeta< 'r > {

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

            self.known_maps.push_new(
                Url::from( guess_sitemap( &self.base_url ) ).into( )
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
    //TODO: let response ... is ugly brittle. No error handling
    #[inline]
    fn fetch_next_map( &mut self ) -> bool {
        let next_entry;
        if { next_entry = self.known_maps.pop( ); next_entry.is_some( ) } {
            let response = self.client.get::<Url>( next_entry.clone( ).unwrap( )
                                                   .loc.get_url( ).unwrap( ) ).send( );
            match response {
                Ok( r ) => {
                    self.curr_map = Some( (
                        SiteMapReader::< Response >::new( r ),
                        next_entry.unwrap( ) ) );
                    return true;
                }
                Err( _e ) => {
                    self.known_maps.push_stale( next_entry.unwrap( ) );
                    //TODO: Here be dragons
                }
            }
        }
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
                if self.known_maps.is_stale( ) { return false; }
            }
        }
        true
    }

    //TODO: .expect( ) is brittle
    //TODO: sitemap.xml CAN contain relative path URLs
    fn next_in_map( &mut self ) -> Option< BaseUrl > {

        let ( mut curr_map, map_entry ) = self.curr_map.take( ).unwrap( );
        let mut w_entry = curr_map.next( );

        while w_entry.is_some( ) {
            let entry = w_entry.unwrap( );
            match entry {

                SiteMapEntity::Url( ue ) => {
                    match ue.loc {
                        Location::Url( u ) => {
                            self.curr_map.get_or_insert( ( curr_map, map_entry ) );
                            return Some( BaseUrl::try_from( u )
                                         .expect( "Sitemap contains an invalid url" ) );
                        }


                        _ => { /*Do Nothing*/ }
                    }
                }

                SiteMapEntity::SiteMap( se ) => {
                    match se.loc {
                        Location::Url( u ) => {
                            self.known_maps.push_new(
                                SiteMapEntry::from( BaseUrl::try_from( u )
                                                    .expect( "Sitemap contains an invalid url" ) )
                            );
                        }
                        _ => { /*Do Nothing*/}
                    }
                }

                _ => { /*Do Nothing*/ }
            }
            w_entry = curr_map.next( );
        }

        self.known_maps.push_stale( map_entry );
        None
    }
}

impl< 'r > Iterator for SiteMeta< 'r > {
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

impl< 'r > SiteMeta< 'r > {

    /// Builds a SiteMeta structure from a BaseUrl pointing to a robots.txt file on a server somewhere.
    ///
    /// # Errors:
    /// If an error occurs trying to fetch the robots.txt file it is returned instead. See
    /// [Link](https://docs.rs/reqwest/0.8.6/reqwest/struct.Error.html) for more.
    pub fn from_robots_url( robots_url:&BaseUrl ) -> Result< ( SiteMeta< 'r > ), ReqwestError > {
        let client = Client::new( ); //TODO: add client setup
        let response = client.get::< Url >( robots_url.clone( ).into( ) ).send( );
        let robots_txt: RobotFileParser< 'r > = RobotFileParser::< 'r >::from( robots_url.clone( ) );
        let known_maps: MapsList;
        let mut host = robots_url.clone( );
        host.set_path( "/" );

        match response{
            Ok( mut r ) => {
                robots_txt.from_response( &mut r );
            }
            Err( e ) => {
                return Err( e );
            }
        };

        known_maps = robots_txt.get_sitemaps( "you're cute!" )
            .into_iter( )
            .collect( );
        println!( "{:?}", robots_txt );
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
    pub fn from_url( url:&BaseUrl ) -> Result< ( SiteMeta< 'r > ), ReqwestError > {
        Self::from_robots_url( &guess_robots( url ) )
    }

}

