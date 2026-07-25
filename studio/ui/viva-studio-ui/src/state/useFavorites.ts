import { useCallback, useState } from "react";

const STORAGE_KEY = "viva-studio:favorites";

function loadFavorites(): Set<string> {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) {
      const arr = JSON.parse(stored);
      if (Array.isArray(arr)) return new Set(arr);
    }
  } catch {
    // Ignore corrupt storage
  }
  return new Set();
}

function saveFavorites(favorites: Set<string>) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify([...favorites]));
}

export function useFavorites() {
  const [favorites, setFavorites] = useState<Set<string>>(loadFavorites);

  const toggleFavorite = useCallback((name: string) => {
    setFavorites((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      saveFavorites(next);
      return next;
    });
  }, []);

  return { favorites, toggleFavorite };
}
