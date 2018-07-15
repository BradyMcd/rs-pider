///This is a collection of small utility functions for sanitizing Urls from unknown sources

use url::Url;
pub use mapper::MapperError;

fn strip( url: &mut Url ) -> Option< MapperError > {
    if url.set_scheme( "https" ).is_ok( ) &&
        url.set_port( None ).is_ok( ) &&
        url.set_username( "" ).is_ok( ) &&
        url.set_password( None ).is_ok( ) {
            url.set_path( "" );
            url.set_query( None );
            url.set_fragment( None );
            None
    } else {
        Some( MapperError::ImpossibleError )
    }
}

///mutates a Url, stripping all information outside of host and protocol
pub fn to_base_url( url: &mut Url ) -> Option< MapperError > {
    if url.cannot_be_a_base( ) {
        return Some( MapperError::InvalidHost );
    }
    return strip( url );
}

///borrows a Url and returns a clone stripped of all information outside of host and protocol
pub fn base_url( url:&Url ) -> Result< Url, MapperError > {
    if url.cannot_be_a_base( ) {
        return Err( MapperError::InvalidHost );
    }
    let mut host = url.clone( );
    return match strip( &mut host ) {
        Some( e ) => Err( e ),
        None => Ok( host )
    };
}

///borrows a Url and returns true if it is a suitable base url with an empty path and no further information
pub fn is_host_only( url:&Url ) -> Result< bool, MapperError > {
    if url.cannot_be_a_base( ) {
        return Err( MapperError::InvalidHost );
    }
    Ok( url.has_authority( ) && url.path( ) == "/" )
}
