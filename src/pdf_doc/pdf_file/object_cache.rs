use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::{Rc, Weak};

use super::{PdfObject, PdfObjectInterface, ObjectId, PdfFileReader, ParserInterface, SharedObject, parse_object_at};
use crate::errors::*;

#[derive(Debug)]
pub struct ObjectCache {
    cache: RefCell<HashMap<ObjectId, Rc<PdfObject>>>,
    index_map: RefCell<HashMap<ObjectId, ObjectLocation>>,
    reader: PdfFileReader,
    self_ref: RefCell<Weak<Self>>
}

#[derive(Debug, Clone, Copy)]
pub enum ObjectLocation {
    Uncompressed(usize),
    Compressed(ObjectId)
}

impl ObjectCache {
    pub fn new(reader: PdfFileReader, index: HashMap<ObjectId, ObjectLocation>, weak_ref: Weak<Self>) -> Self {
        ObjectCache{
            cache: RefCell::new(HashMap::new()),
            index_map: RefCell::new(index),
            reader,
            self_ref: RefCell::new(weak_ref)
        }
    }
    pub fn update_reference(&self, new_ref: Weak<Self>) {
        self.self_ref.replace(new_ref);
    }
    pub fn update_index(&self, new_index: HashMap<ObjectId, ObjectLocation>) {
        *self.index_map.borrow_mut() = new_index;
    }
    pub fn reader(&self) -> PdfFileReader {
        self.reader.spawn_clone()
    }
    pub fn get_object_list(&self) -> Vec<ObjectId> {
        self.index_map.borrow().iter().map(|(a, _)| *a).collect()
    }
}

impl ParserInterface<PdfObject> for ObjectCache {
    fn retrieve_object_by_ref(&self, id: ObjectId) -> Result<SharedObject> {
        
        trace!("retrieving object# {}", id);
        let cache_results;
        {
            let map = self.cache.borrow_mut();
            cache_results = map.get(&id).map(|r| Rc::clone(r));
        } // Drop borrow of cache here, before potentially recursive call to parse_object_at

        use ObjectLocation::*;
        if let None = cache_results {
            let new_obj = match self.index_map.borrow().get(&id) {
                None => Err(ErrorKind::ReferenceError(format!("Object #{} does not exist", id)))?,
                Some(Uncompressed(ix)) => Rc::new(parse_object_at(
                    self.reader.spawn_clone(), *ix, &Weak::clone(&self.self_ref.borrow()))?.0),
                Some(Compressed(parent_id)) => {
                    let parent = self.retrieve_object_by_ref(*parent_id)?;
                    parent.try_into_object_stream()?.retrieve_object_by_ref(id)?
                }
            };
            let mut map = self.cache.borrow_mut();  // Mutable borrow of map
            map.insert(id, new_obj);
        };  // Mutable borrow of map dropped here
        Ok(Rc::clone(self.cache.borrow().get(&id).unwrap()))  // Immutable borrow of map

    }
    fn retrieve_trailer(&self) -> Result<&PdfObject> {
        Err(ErrorKind::UnavailableType("trailer".to_string(), "retrieve_trailer".to_string()).into())
    }
}

#[derive(Clone, Debug)]
pub struct ObjectStreamCache {
    cache: RefCell<HashMap<ObjectId, Rc<PdfObject>>>,
    index_map: HashMap<ObjectId, usize>,
    reader: PdfFileReader,
    reader_is_decoded: bool,
    master_cache_ref: Weak<ObjectCache>
}

impl ParserInterface<PdfObject> for ObjectStreamCache {
    fn retrieve_object_by_ref(&self, id: ObjectId) -> Result<SharedObject> {
        
        trace!("retrieving object# {}", id);
        let cache_results;
        {
            let map = self.cache.borrow_mut();
            cache_results = map.get(&id).map(|r| Rc::clone(r));
        } // Drop borrow of cache here, before potentially recursive call to parse_object_at

        {
            let object_ix = self.index_map.get(&id)
                                      .ok_or(ErrorKind::ReferenceError(format!("Object #{} does not exist", id)))?;
            let (new_obj, _) =  parse_object_at(
                self.reader.spawn_clone(), *object_ix, &Weak::clone(&self.master_cache_ref))?;
            let mut map = self.cache.borrow_mut();  // Mutable borrow of map
            map.insert(id, Rc::new(new_obj));
        };  // Mutable borrow of map dropped here

        Ok(Rc::clone(self.cache.borrow().get(&id).unwrap()))  // Immutable borrow of map

    }
    fn retrieve_trailer(&self) -> Result<&PdfObject> {
        Err(ErrorKind::UnavailableType("trailer".to_string(), "retrieve_trailer".to_string()).into())
    }
}

impl fmt::Display for ObjectStreamCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Object stream")
    }
}