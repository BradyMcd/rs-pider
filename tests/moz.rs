
extern crate rs_pider;
extern crate base_url;

use base_url::{ BaseUrl, Url };
use rs_pider::meta::SiteMeta;

fn main() {

    let _url = Url::parse( "https://www.mozilla.org/robots.txt" ).unwrap( );
    let url = BaseUrl::from( _url );

    let meta = match SiteMeta::from_robots_url( &url ) {
        Ok( sm ) => sm,
        Err( _e ) => panic!( _e ),
    };

    for map in meta {
        println!( "{}", map );
    }
}
