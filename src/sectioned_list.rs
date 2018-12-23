//
extern crate cuckoofilter;

use std::collections::hash_map::DefaultHasher;

use self::cuckoofilter::CuckooFilter;
use std::collections::VecDeque;

pub struct SectionedList< T > {
    main: VecDeque< T >,
    alt: VecDeque< T >,
    filter: CuckooFilter<DefaultHasher>,
}

impl< T: Eq + std::hash::Hash > SectionedList< T > {
    pub fn new( ) -> Self {
        SectionedList{
            main: VecDeque::new( ),
            alt: VecDeque::new( ),
            filter: CuckooFilter::new( ),
        }
    }

    pub fn is_empty( &self ) -> bool {
        self.main.is_empty( ) && self.alt.is_empty( )
    }

    pub fn is_alt( &self ) -> bool {
        self.main.is_empty( ) && !self.alt.is_empty( )
    }

    pub fn push_main( &mut self, data: T ) {
        if self.filter.test_and_add( &data ) {
            self.main.push_back( data );
        }
    }

    pub fn peek_main( &self ) -> Option< &T > {
        self.main.front( )
    }

    pub fn skip_main( &mut self ) {
        match self.main.pop_front( ) {
            Some( d ) => self.alt.push_back( d ),
            None => { /* DO NOTHING */ },
        }
    }

    pub fn len( &self ) -> usize {
        self.main.len()
    }

    pub fn merge( &mut self ) {
        self.main.append( &mut self.alt );
    }

    pub fn filtered_merge( &mut self, filter: &mut FnMut( &T ) -> bool ) {
        let ( m, a ):( Vec<T>, Vec< T > ) = self.alt.drain( .. ).partition( filter );

        self.main.extend( m );
        self.alt.extend( a );
    }

}
