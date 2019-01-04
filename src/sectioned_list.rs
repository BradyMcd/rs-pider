//
//TODO: Size invariants must be added, the sum of two VecDeques will break len
extern crate cuckoofilter;

use std::collections::hash_map::DefaultHasher;
use std::iter::FromIterator;

use self::cuckoofilter::CuckooFilter;
use std::collections::VecDeque;

pub struct SectionedList< T > {
    main: VecDeque< T >,
    alt: VecDeque< T >,
    filter: CuckooFilter<DefaultHasher>,
    #[cfg( feature = "diag" )]
    false_positive: usize,
}

impl< U, T: PartialEq + std::hash::Hash + From< U > > FromIterator< U > for SectionedList< T > {
    fn from_iter< I:IntoIterator< Item = U > > ( iter: I ) -> Self {
        let mut ret = SectionedList::new( );
        for item in iter {
            ret.push_main( item.into( ) );
        }
        ret
    }
}

impl< T: PartialEq + std::hash::Hash > SectionedList< T > {
    pub fn new( ) -> Self {
        SectionedList{
            main: VecDeque::new( ),
            alt: VecDeque::new( ),
            filter: CuckooFilter::new( ),
            #[cfg( feature = "diag" )]
            false_positive: 0,
        }
    }

    pub fn is_empty( &self ) -> bool {
        self.main.is_empty( ) && self.alt.is_empty( )
    }

    pub fn is_alt( &self ) -> bool {
        self.main.is_empty( ) && !self.alt.is_empty( )
    }

    #[inline]
    #[cfg( feature = "diag" ) ]
    fn add_false_positive( &self ) {
        self.false_positive = self.false_positive + 1;
    }

    #[inline]
    #[cfg( not( feature = "diag" ))]
    fn add_false_positive( &self ) { /* Do Nothing */ }

    pub fn push_main( &mut self, data: T ) {
        if !self.filter.contains( &data ) {
            self.filter.add( &data ); //possible error
            self.main.push_back( data );
        } else if !self.main.iter( ).any( | d |{ *d == data } ) &&
            !self.alt.iter( ).any( | d |{ *d == data } ) {
                self.add_false_positive( );
                self.main.push_back( data );
        }
    }

    pub fn peek_main( &mut self ) -> Option< &mut T > {
        self.main.front_mut( )
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
