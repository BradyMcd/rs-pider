
use url::Url;
pub use mapper::MapperError;
///takes a url and ensures it is in the form https://foo.bar/ and is a valid host
pub fn to_base_url( mut url:Url ) -> Result< Url, MapperError >{
    if url.cannot_be_a_base( ) {
        return Err( MapperError::InvalidHost );
    }
    url.set_scheme( "https" ).expect( "The Impossible happened" );
    url.set_path( "" );
    url.set_port( None ).expect( "The Impossible happened" );
    url.set_query( None );
    url.set_fragment( None );
    url.set_username( "" ).expect( "The Impossible happened" );
    url.set_password( None ).expect( "The Impossible happened" );
    Ok( url )
}
