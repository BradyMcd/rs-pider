//
//TODO: Tests sort of desperately must be writtem
extern crate cuckoofilter;

use std::collections::hash_map::DefaultHasher;
use std::iter::FromIterator;

use self::cuckoofilter::CuckooFilter;

struct QLink <T> {
    next: Option< usize >,
    data: T
}

impl< T > QLink< T > {

    fn new( data: T ) -> Self {
        QLink{
            next: None,
            data: data,
        }
    }

    fn to( &mut self, idx: usize ) {
        assert!( self.next.is_none( ) );

        self.next = Some( idx );
    }

    fn has_next( &self ) -> bool {
        self.next.is_some( )
    }

    fn idx( &self ) -> usize {
        self.next.unwrap( )
    }

    fn break_link( &mut self ) {
        self.next = None;
    }

    fn move_link( &mut self ) -> Option< usize > {
        self.next.take( )
    }

    fn get_mut( &mut self ) -> &mut T {
        &mut self.data
    }

    fn get( &self ) -> &T {
        &self.data
    }
}

struct Backing< T > {
    buffer: Vec< QLink< T > >,
    fronts: Vec< Option< usize > >,
    backs: Vec< Option< usize > >,
    // Until I develop some enum tooling to accomplish this more cleanly, a sections value of 0 refers
    // to an "infinite" (usize::MAX) number of sections
    sections: usize,
}

impl< T: PartialEq > Backing< T > {


    fn new( sections: usize ) -> Self {
        assert!( sections > 1 || sections == 0 );
        Backing {
            buffer: Vec::new( ),
            fronts: Vec::new( ),
            backs: Vec::new( ),
            sections: sections,
        }
    }

    fn link_level_push( &mut self, section: usize, idx: usize ) {
        assert!( section <= self.fronts.len( ) );
        assert!( section < self.sections || self.sections == 0 );

        if section == self.fronts.len( ) { // new section
            self.fronts.push( Some( idx ) );
            self.backs.push( Some( idx ) );

        } else if self.fronts[ section ].is_none( ) { // empty section
            self.fronts[ section ] = Some( idx );
            self.backs[ section ] = Some( idx );

        } else { // general case
            let old_back_idx = self.backs[ section ].unwrap( );
            self.buffer[ old_back_idx ].to( idx );
            self.backs[ section ] = Some( idx );
        }
    }

    fn push_to_section( &mut self, data: T, section: usize ) {
        assert!( section < self.fronts.len( ) );
        assert!( section < self.sections || self.sections == 0 );

        let add_idx = self.buffer.len( );
        self.buffer.push( QLink::new( data ) );
        self.link_level_push( section, add_idx );
    }

    fn push( &mut self, data:T ) {

        self.push_to_section( data, 0 );
    }

    fn peek_section_mut( &mut self, section: usize ) -> Option< &mut T > {
        assert!( section < self.fronts.len( ) );

        if self.fronts[section].is_none( ) { return None; }

        Some( self.buffer[ self.fronts[section].unwrap( ) ].get_mut( ) )
    }

    fn peek_mut( &mut self ) -> Option < &mut T > {
        self.peek_section_mut( 0 )
    }

    fn peek_section( &self, section: usize ) -> Option< &T > {
        assert!( section < self.fronts.len( ) );

        if self.fronts[ section ].is_none( ) { return None; }

        Some( self.buffer[ self.fronts[ section ].unwrap( ) ].get( ) )
    }

    fn peek( &self ) -> Option< &T > {
        self.peek_section( 0 )
    }

    fn advance_section( &mut self, section: usize ) {
        assert!( section < self.sections || self.sections == 0 );

        if section >= self.fronts.len( ) {
            return; /* DO NOTHING */
        }

        let target_section = if section + 1 == self.sections {
            0
        } else {
            section + 1
        };

        if self.fronts[section].is_none( ) { return; /* DO NOTHING */ }

        if self.fronts[section] == self.backs[section] { //last element in section
            let old_front = self.fronts[ section ].take( );
            self.link_level_push( target_section, old_front.unwrap( ) );
            self.backs[ section ] = None;

        } else { //general case
            let old_front_idx = self.fronts[ section ].unwrap( );

            self.fronts[ section ] = self.buffer[ old_front_idx ].move_link( );
            self.link_level_push( target_section , old_front_idx );
        }
    }

    fn advance( &mut self ) {
        self.advance_section( 0 );
    }

    fn merge_sections( &mut self, a: usize, b: usize ) {
        assert!( ( a < self.sections && b < self.sections ) || self.sections == 0 );
        if a >= self.fronts.len( ) || b >= self.fronts.len( ) {
            return;
        }

        if self.fronts[ b ].is_some( ) {
            if self.fronts[ a ].is_some( ) {
                let old_back_idx = self.backs[ a ].unwrap( );
                self.buffer[ old_back_idx ].to( self.fronts[ b ].take( ).unwrap( ) );
                self.backs[ a ] = self.backs[ b ].take( );

            } else {
                self.fronts[ a ] = self.fronts[ b ].take( );
                self.backs[ a ] = self.backs[ b ].take( );
            }
        }
    }

    fn merge( &mut self ) {
        for i in ( 2..self.sections ).rev( ) {
            self.merge_sections( i-2, i-1 );
        }
    }

    fn is_empty( &self ) -> bool {
        self.buffer.is_empty( )
    }

    fn len( &self ) -> usize {
        self.buffer.len( )
    }

    fn check_section( &self, section: usize ) -> bool {
        assert!( section < self.sections || self.sections == 0 );

        if section >= self.fronts.len( ) {
            return false;
        }

        self.fronts[ section ].is_some( )
    }

    fn contains( &self, data: &T ) -> bool {
        self.buffer.iter( ).any( | d | { ( d.get( ) == data ) } )
    }
}

pub struct SectionedList< T > {
    _intern: Backing<T>,
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
            _intern: Backing::new( 2 ),
            filter: CuckooFilter::new( ),
            #[cfg( feature = "diag" )]
            false_positive: 0,
        }
    }

    pub fn is_empty( &self ) -> bool {
        self._intern.is_empty( )
    }

    pub fn has_stale( &self ) -> bool {
        self._intern.check_section( 1 )
    }

    pub fn has_main( &self ) -> bool {
        self._intern.check_section( 0 )
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
            self._intern.push( data );
        } else if !self._intern.contains( &data ){
            self._intern.push( data );
            self.add_false_positive( )
        }
    }

    pub fn peek_main( &mut self ) -> Option< &mut T > {
        self._intern.peek_mut( )
    }

    pub fn skip_main( &mut self ) {
        self._intern.advance( )
    }

    pub fn len( &self ) -> usize {
        self._intern.len( )
    }

    pub fn merge( &mut self ) {
        self._intern.merge( );
    }

    //TODO: Filtered merge is slightly harder with this linking structure, maybe some form of
    // partitioning would be a better approach, this will break ordering
}

mod tests {
    use super::*;

    #[test]
    fn test_sectioned_something( ) {

    }

}
