///This is a collection of small utility functions for sanitizing Urls from unknown sources

use url::Url;

pub enum UrlError {

    InvalidHost,
    /// If you ever see this something is very wrong with the environment or some undocumented error
    /// occurred
    ImpossibleError,
}

fn strip( url: &mut Url ) -> Option< UrlError > {
    if url.set_scheme( "https" ).is_ok( ) &&
        url.set_port( None ).is_ok( ) &&
        url.set_username( "" ).is_ok( ) &&
        url.set_password( None ).is_ok( ) {
            url.set_path( "" );
            url.set_query( None );
            url.set_fragment( None );
            None
    } else {
        Some( UrlError::ImpossibleError )
    }
}

///mutates a Url, stripping all information outside of host and protocol
pub fn to_base_url( url: &mut Url ) -> Option< UrlError > {
    if url.cannot_be_a_base( ) {
        return Some( UrlError::InvalidHost );
    }
    return strip( url );
}

///borrows a Url and returns a clone stripped of all information outside of host and protocol
pub fn base_url( url:&Url ) -> Result< Url, UrlError > {
    if url.cannot_be_a_base( ) {
        return Err( UrlError::InvalidHost );
    }
    let mut host = url.clone( );
    return match strip( &mut host ) {
        Some( e ) => Err( e ),
        None => Ok( host )
    };
}

///borrows a Url and returns true if it is a suitable base url with an empty path and no further information
pub fn is_host_only( url:&Url ) -> Result< bool, UrlError > {
    if url.cannot_be_a_base( ) {
        return Err( UrlError::InvalidHost );
    }
    Ok( url.has_authority( ) && url.path( ) == "/" )
}
