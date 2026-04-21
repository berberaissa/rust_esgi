/// Slab allocator core — inspiré du SLUB de Linux.
///
/// Architecture :
/// - 9 classes de taille fixes (`SLAB_SIZES`).
/// - Chaque classe est gérée par un [`SlabCache`] avec une liste libre
///   intrusive (les blocs libres stockent le pointeur "suivant" dans leurs
///   propres octets — zéro surcharge mémoire).
/// - Quand un cache est vide, il demande une page de 4 Kio au
///   [`BumpAllocator`] interne et la découpe en blocs (`refill`).
/// - Les allocations > 2048 octets sont servies directement par le
///   [`BumpAllocator`] (fallback).
use core::alloc::Layout;
use core::mem;
use core::ptr;

// ── Constantes ────────────────────────────────────────────────────────────────

/// Classes de taille disponibles (en octets).
///
/// Toute demande d'allocation est arrondie à la plus petite classe
/// qui satisfait à la fois la taille **et** l'alignement.
/// Au-delà de 2048 octets → fallback bump allocator.
pub const SLAB_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];

/// Nombre de classes. Doit correspondre à `SLAB_SIZES.len()`.
const NUM_SLAB_SIZES: usize = 9;

/// Taille d'une page slab (4 Kio).
const SLAB_PAGE_SIZE: usize = 4096;

// ── FreeNode ──────────────────────────────────────────────────────────────────

/// Nœud de la liste libre intrusive.
///
/// Quand un bloc est **libre**, ses premiers octets contiennent un pointeur
/// vers le bloc libre suivant. Quand il est **alloué**, ces octets sont
/// écrasés par les données de l'utilisateur.
///
/// C'est le même mécanisme que Linux SLUB : zéro mémoire supplémentaire
/// pour la gestion des blocs libres.
struct FreeNode {
    next: Option<&'static mut FreeNode>,
}

// ── SlabCache ─────────────────────────────────────────────────────────────────

/// Cache gérant un pool d'objets de taille fixe.
///
/// Chaque `SlabCache` sert les allocations d'une seule *classe de taille*.
/// Les blocs libres sont chaînés de façon intrusive (voir [`FreeNode`]).
///
/// Quand la liste est vide, le cache demande une page de 4 Kio au
/// [`BumpAllocator`] via [`allocate`](SlabCache::allocate) et la découpe
/// en blocs (`refill`).
pub struct SlabCache {
    /// Taille en octets de chaque bloc géré par ce cache.
    block_size: usize,
    /// Tête de la liste libre (`None` = liste vide).
    free_list: Option<&'static mut FreeNode>,
}

impl SlabCache {
    /// Crée un cache vide pour des blocs de `block_size` octets.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rust_esgi::allocator::slab::SlabCache;
    /// let cache = SlabCache::new(64);
    /// assert_eq!(cache.block_size(), 64);
    /// ```
    pub const fn new(block_size: usize) -> Self {
        SlabCache { block_size, free_list: None }
    }

    /// Retourne la taille des blocs gérés par ce cache.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rust_esgi::allocator::slab::SlabCache;
    /// assert_eq!(SlabCache::new(128).block_size(), 128);
    /// ```
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    // ── private helpers ───────────────────────────────────────────────────────

    /// Dépile le premier bloc libre de la liste (O(1)).
    fn pop(&mut self) -> Option<*mut u8> {
        self.free_list.take().map(|node| {
            self.free_list = node.next.take();
            node as *mut FreeNode as *mut u8
        })
    }

    /// Empile un bloc libre en tête de liste (O(1)).
    ///
    /// # Safety
    ///
    /// `ptr` doit pointer vers au moins `block_size` octets valides,
    /// exclusivement possédés par l'appelant.
    unsafe fn push(&mut self, ptr: *mut u8) {
        let node = ptr as *mut FreeNode;
        node.write(FreeNode { next: self.free_list.take() });
        self.free_list = Some(&mut *node);
    }

    /// Découpe une page de `SLAB_PAGE_SIZE` octets en blocs et les pousse
    /// tous dans la liste libre.
    ///
    /// L'itération en sens inverse fait que le bloc 0 se retrouve en tête
    /// → ordre LIFO → accès cache-chaud.
    ///
    /// # Safety
    ///
    /// `page_ptr` doit pointer vers exactement `SLAB_PAGE_SIZE` octets
    /// valides, écrits, non aliasés.
    unsafe fn refill(&mut self, page_ptr: *mut u8) {
        let num_blocks = SLAB_PAGE_SIZE / self.block_size;
        for i in (0..num_blocks).rev() {
            let block = page_ptr.add(i * self.block_size);
            self.push(block);
        }
    }

    // ── API publique d'allocation ─────────────────────────────────────────────

    /// Alloue un bloc depuis ce cache.
    ///
    /// Chemin rapide : pop de la liste libre (O(1)).
    /// Chemin lent : demande une page au `bump`, la découpe en blocs, puis pop.
    ///
    /// Retourne un **pointeur nul** si le bump allocator est épuisé.
    ///
    /// # Safety
    ///
    /// `bump` doit être un [`BumpAllocator`] initialisé valide.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let ptr = unsafe { cache.allocate(&mut bump) };
    /// assert!(!ptr.is_null());
    /// ```
    pub unsafe fn allocate(&mut self, bump: &mut BumpAllocator) -> *mut u8 {
        match self.pop() {
            Some(ptr) => ptr,
            None => match bump.alloc_page() {
                Some(page) => {
                    self.refill(page);
                    self.pop().unwrap_or(ptr::null_mut())
                }
                None => ptr::null_mut(),
            },
        }
    }

    /// Libère un bloc et le remet en tête de la liste libre.
    ///
    /// Le bloc sera immédiatement réutilisable par le prochain appel à
    /// [`allocate`](SlabCache::allocate).
    ///
    /// # Safety
    ///
    /// `ptr` doit avoir été retourné par [`allocate`](SlabCache::allocate)
    /// **sur ce même cache** et ne doit plus être utilisé après cet appel.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// unsafe {
    ///     let ptr = cache.allocate(&mut bump);
    ///     cache.deallocate(ptr); // bloc remis dans la liste
    /// }
    /// ```
    pub unsafe fn deallocate(&mut self, ptr: *mut u8) {
        self.push(ptr);
    }
}

// ── BumpAllocator ─────────────────────────────────────────────────────────────

/// Allocateur bump (linéaire) utilisé comme source de pages slab et comme
/// fallback pour les allocations > 2048 octets.
///
/// La mémoire est distribuée en avançant un pointeur (`next`) dans une
/// région virtuelle. Les libérations sont ignorées — la mémoire bump
/// n'est jamais récupérée. C'est acceptable car les grandes allocations
/// directes sont rares dans le code noyau.
///
/// # Limitations
///
/// * Les grandes allocations ne sont jamais libérées.
/// * Non adapté comme allocateur général autonome.
pub struct BumpAllocator {
    /// Adresse de début du heap (incluse).
    heap_start: usize,
    /// Adresse de fin du heap (exclue).
    heap_end: usize,
    /// Prochaine adresse libre (avance toujours).
    next: usize,
    /// Nombre total d'allocations effectuées (debug / stats).
    allocations: usize,
}

impl BumpAllocator {
    /// Crée un allocateur bump **non initialisé**.
    ///
    /// Appeler [`init`](BumpAllocator::init) avant toute allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rust_esgi::allocator::slab::BumpAllocator;
    /// let bump = BumpAllocator::new();
    /// assert_eq!(bump.allocations(), 0);
    /// ```
    pub const fn new() -> Self {
        BumpAllocator { heap_start: 0, heap_end: 0, next: 0, allocations: 0 }
    }

    /// Initialise l'allocateur avec la région `heap_start..heap_start + heap_size`.
    ///
    /// # Safety
    ///
    /// La plage entière doit être valide, accessible en écriture, et non
    /// aliasée. Cette fonction doit être appelée au plus une fois.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end   = heap_start + heap_size;
        self.next       = heap_start;
    }

    /// Alloue un bloc satisfaisant `layout`.
    ///
    /// Retourne un **pointeur nul** si le heap est épuisé ou en cas d'overflow.
    ///
    /// # Safety
    ///
    /// Doit être appelé après [`init`](BumpAllocator::init).
    pub unsafe fn alloc(&mut self, layout: Layout) -> *mut u8 {
        let start = align_up(self.next, layout.align());
        let end   = match start.checked_add(layout.size()) {
            Some(e) => e,
            None    => return ptr::null_mut(),
        };
        if end > self.heap_end {
            return ptr::null_mut();
        }
        self.next = end;
        self.allocations += 1;
        start as *mut u8
    }

    /// Alloue une page de 4 Kio alignée sur 4 Kio.
    ///
    /// Utilisé par [`SlabCache::allocate`] pour la croissance de slab.
    pub unsafe fn alloc_page(&mut self) -> Option<*mut u8> {
        let layout = Layout::from_size_align(SLAB_PAGE_SIZE, SLAB_PAGE_SIZE)
            .expect("layout de page toujours valide");
        let ptr = self.alloc(layout);
        if ptr.is_null() { None } else { Some(ptr) }
    }

    /// Retourne le nombre total d'allocations effectuées.
    ///
    /// Utile pour les statistiques et le debug. Les libérations ne
    /// décrémentent pas ce compteur (le bump ne les traque pas).
    ///
    /// # Examples
    ///
    /// ```
    /// # use rust_esgi::allocator::slab::BumpAllocator;
    /// let bump = BumpAllocator::new();
    /// assert_eq!(bump.allocations(), 0);
    /// ```
    pub fn allocations(&self) -> usize {
        self.allocations
    }
}

// ── Fonction utilitaire ───────────────────────────────────────────────────────

/// Arrondit `addr` au multiple supérieur de `align`.
///
/// `align` **doit** être une puissance de 2 ; tout autre valeur produit un
/// résultat incorrect sans paniquer.
///
/// # Examples
///
/// ```
/// # use rust_esgi::allocator::slab::align_up;
/// assert_eq!(align_up(0,   8),  0);
/// assert_eq!(align_up(1,   8),  8);
/// assert_eq!(align_up(8,   8),  8);
/// assert_eq!(align_up(9,   8), 16);
/// assert_eq!(align_up(17, 16), 32);
/// ```
pub fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

// ── SlabAllocator ─────────────────────────────────────────────────────────────

/// Allocateur slab complet : 9 caches + fallback bump.
///
/// # Fonctionnement (inspiré SLUB)
///
/// 1. `alloc(layout)` calcule la taille effective
///    (`max(size, align).next_power_of_two()`).
/// 2. La plus petite classe ≥ taille effective est sélectionnée.
/// 3. Le [`SlabCache`] correspondant pop un bloc libre (O(1)).
/// 4. Si le cache est vide → croissance slab automatique.
/// 5. Si la taille > 2048 octets → bump allocator directement.
///
/// # Libération
///
/// * Blocs slab → retour dans la liste libre du cache (réutilisable).
/// * Blocs bump (> 2048 o) → no-op (limitation connue du bump).
pub struct SlabAllocator {
    /// Un cache par classe de taille définie dans [`SLAB_SIZES`].
    caches: [SlabCache; NUM_SLAB_SIZES],
    /// Bump allocator : source de pages + fallback grandes allocations.
    fallback: BumpAllocator,
}

impl SlabAllocator {
    /// Crée un `SlabAllocator` **non initialisé**.
    ///
    /// Appeler [`init`](SlabAllocator::init) avant toute utilisation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rust_esgi::allocator::slab::SlabAllocator;
    /// let alloc = SlabAllocator::new();
    /// ```
    pub const fn new() -> Self {
        SlabAllocator {
            caches: [
                SlabCache::new(8),
                SlabCache::new(16),
                SlabCache::new(32),
                SlabCache::new(64),
                SlabCache::new(128),
                SlabCache::new(256),
                SlabCache::new(512),
                SlabCache::new(1024),
                SlabCache::new(2048),
            ],
            fallback: BumpAllocator::new(),
        }
    }

    /// Initialise l'allocateur avec la région heap `heap_start..+heap_size`.
    ///
    /// # Safety
    ///
    /// La plage doit être valide, accessible en écriture, et exclusivement
    /// possédée. Doit être appelée exactement une fois avant toute allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rust_esgi::allocator::slab::SlabAllocator;
    /// let mut alloc = SlabAllocator::new();
    /// // unsafe { alloc.init(HEAP_START, HEAP_SIZE); }
    /// ```
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.fallback.init(heap_start, heap_size);
    }

    /// Retourne l'index de cache pour `layout`, ou `None` si trop grand.
    ///
    /// La classe sélectionnée est la plus petite dont la taille satisfait
    /// à la fois la taille et l'alignement de `layout`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use core::alloc::Layout;
    /// # use rust_esgi::allocator::slab::SlabAllocator;
    /// // 10 octets, aligné 8 → effective = max(10,8).next_power_of_two() = 16
    /// let l = Layout::from_size_align(10, 8).unwrap();
    /// assert_eq!(SlabAllocator::cache_index(&l), Some(1)); // SLAB_SIZES[1] == 16
    ///
    /// // 4096 octets → trop grand pour les slabs
    /// let l = Layout::from_size_align(4096, 8).unwrap();
    /// assert_eq!(SlabAllocator::cache_index(&l), None);
    /// ```
    pub fn cache_index(layout: &Layout) -> Option<usize> {
        let required  = layout.size().max(layout.align());
        let effective = required
            .next_power_of_two()
            .max(mem::size_of::<FreeNode>());
        SLAB_SIZES.iter().position(|&s| s >= effective)
    }

    /// Alloue de la mémoire satisfaisant `layout`.
    ///
    /// Route vers un [`SlabCache`] ou vers le [`BumpAllocator`] selon la
    /// taille. Retourne un **pointeur nul** en cas d'échec.
    ///
    /// # Safety
    ///
    /// L'allocateur doit avoir été initialisé avec [`init`](SlabAllocator::init).
    pub unsafe fn alloc(&mut self, layout: Layout) -> *mut u8 {
        match Self::cache_index(&layout) {
            Some(idx) => self.caches[idx].allocate(&mut self.fallback),
            None      => self.fallback.alloc(layout),
        }
    }

    /// Libère le bloc à `ptr` décrit par `layout`.
    ///
    /// Pour les blocs slab : retour dans la liste libre du cache (O(1)).
    /// Pour les blocs bump (grandes tailles) : no-op.
    ///
    /// # Safety
    ///
    /// `ptr` et `layout` doivent correspondre exactement à un appel
    /// précédent à [`alloc`](SlabAllocator::alloc) sur cet allocateur.
    pub unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        match Self::cache_index(&layout) {
            Some(idx) => self.caches[idx].deallocate(ptr),
            None      => { /* grandes allocations : pas de libération bump */ }
        }
    }
}
