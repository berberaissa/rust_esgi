/// Enregistrement de l'allocateur global et initialisation du heap.
///
/// Ce module :
/// 1. Définit [`Locked<T>`] — wrapper `spin::Mutex` pour la mutabilité intérieure.
/// 2. Enregistre [`ALLOCATOR`] via `#[global_allocator]` pour que `Box`,
///    `Vec`, `String`, etc. utilisent notre slab allocator.
/// 3. Expose [`init_heap`] qui mappe les pages virtuelles du heap sur des
///    cadres physiques et initialise l'allocateur.
use x86_64::{
    VirtAddr,
    structures::paging::{
        FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
        mapper::MapToError,
    },
};
use core::alloc::{GlobalAlloc, Layout};
use spin::Mutex;

pub mod slab;
use slab::SlabAllocator;

// ── Constantes du heap ────────────────────────────────────────────────────────

/// Adresse virtuelle de départ du heap noyau.
///
/// Choisie arbitrairement dans l'espace virtuel supérieur, hors des zones
/// déjà utilisées par le noyau et le bootloader.
pub const HEAP_START: usize = 0x_4444_4444_0000;

/// Taille du heap noyau : 1 Mio.
///
/// Augmenter si le noyau a besoin de plus de mémoire dynamique.
/// Doit être un multiple de la taille de page (4 Kio).
pub const HEAP_SIZE: usize = 1024 * 1024; // 1 Mio

// ── Locked<T> ─────────────────────────────────────────────────────────────────

/// Wrapper qui fournit la **mutabilité intérieure** via un spinlock [`Mutex`].
///
/// `GlobalAlloc::alloc` prend `&self` (référence partagée), mais notre
/// allocateur doit modifier ses listes libres à chaque appel. `Locked<T>`
/// permet d'obtenir un `&mut T` exclusif depuis un `&Locked<T>` partagé,
/// en utilisant un spinlock.
///
/// # Pourquoi un spinlock ?
///
/// Dans un noyau, il n'y a pas de scheduleur au moment où l'allocateur est
/// utilisé. Le spinlock boucle activement jusqu'à obtenir le verrou —
/// pas besoin de dormir/réveil.
///
/// # Examples
///
/// ```
/// # use rust_esgi::allocator::Locked;
/// # use rust_esgi::allocator::slab::SlabAllocator;
/// let locked = Locked::new(SlabAllocator::new());
/// // locked.lock() retourne un MutexGuard<SlabAllocator>
/// ```
pub struct Locked<T> {
    inner: Mutex<T>,
}

impl<T> Locked<T> {
    /// Enveloppe `inner` dans un `Mutex` spin.
    ///
    /// `const fn` : utilisable dans un contexte `static`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rust_esgi::allocator::Locked;
    /// # use rust_esgi::allocator::slab::SlabAllocator;
    /// static ALLOC: Locked<SlabAllocator> = Locked::new(SlabAllocator::new());
    /// ```
    pub const fn new(inner: T) -> Self {
        Locked { inner: Mutex::new(inner) }
    }

    /// Acquiert le spinlock et retourne un guard vers la valeur interne.
    ///
    /// Boucle (spin) jusqu'à ce que le verrou soit disponible.
    /// Le verrou est relâché automatiquement quand le guard est droppé.
    pub fn lock(&self) -> spin::MutexGuard<'_, T> {
        self.inner.lock()
    }
}

// ── Allocateur global ─────────────────────────────────────────────────────────

/// L'allocateur slab global du noyau.
///
/// Enregistré avec `#[global_allocator]` : tout type heap (`Box`, `Vec`,
/// `String`, `Arc`, …) passe par cet allocateur.
///
/// Non initialisé au démarrage. [`init_heap`] **doit** être appelé
/// une seule fois dans `kernel_main` avant toute allocation heap.
#[global_allocator]
static ALLOCATOR: Locked<SlabAllocator> = Locked::new(SlabAllocator::new());

unsafe impl GlobalAlloc for Locked<SlabAllocator> {
    /// Alloue un bloc de mémoire satisfaisant `layout`.
    ///
    /// Appelé automatiquement par Rust pour chaque `Box::new`,
    /// `Vec::push`, etc.
    ///
    /// Retourne un **pointeur nul** si le heap est épuisé.
    ///
    /// # Safety
    ///
    /// Respecte le contrat de [`core::alloc::GlobalAlloc::alloc`].
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.lock().alloc(layout)
    }

    /// Libère le bloc à `ptr` décrit par `layout`.
    ///
    /// # Safety
    ///
    /// `ptr` et `layout` doivent correspondre exactement à un appel
    /// précédent à `alloc`.
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.lock().dealloc(ptr, layout)
    }
}

// ── Initialisation du heap ────────────────────────────────────────────────────

/// Mappe les pages virtuelles du heap sur des cadres physiques et
/// initialise l'allocateur slab global.
///
/// # Ce que fait cette fonction
///
/// 1. Itère sur toutes les pages de 4 Kio dans `HEAP_START..HEAP_START+HEAP_SIZE`.
/// 2. Alloue un cadre physique pour chaque page et le mappe `PRESENT | WRITABLE`.
/// 3. Appelle `ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE)` pour donner
///    la région virtuelle à l'allocateur slab.
///
/// # Quand l'appeler
///
/// **Une seule fois**, au tout début de `kernel_main`, après l'initialisation
/// du mapper et du frame allocator, **avant** tout `Box::new()` ou `Vec::new()`.
///
/// # Errors
///
/// Retourne `Err(MapToError)` si une page n'a pas pu être mappée.
///
/// # Examples
///
/// ```no_run
/// rust_esgi::allocator::init_heap(&mut mapper, &mut frame_allocator)
///     .expect("initialisation du heap échouée");
/// ```
pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {

    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end   = VirtAddr::new((HEAP_START + HEAP_SIZE - 1) as u64);
        let start_page = Page::containing_address(heap_start);
        let end_page   = Page::containing_address(heap_end);
        Page::range_inclusive(start_page, end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        // Safety : le cadre vient d'être alloué et n'est pas encore mappé.
        unsafe { mapper.map_to(page, frame, flags, frame_allocator)?.flush(); }
    }

    // Safety : la plage virtuelle est maintenant entièrement mappée et
    // exclusivement possédée par l'allocateur.
    unsafe { ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE); }

    Ok(())
}
