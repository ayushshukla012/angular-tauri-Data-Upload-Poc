import { Injectable } from '@angular/core';
import { DraftState, AttachedDocument } from '../models';

const DB_NAME = 'insight-data-upload-ui';
const STORE = 'drafts';
const KEY = 'current';

export interface StoredDraftBundle {
  draft: DraftState;
  files: Record<string, { name: string; type: string; lastModified: number; file: File }>;
}

@Injectable({ providedIn: 'root' })
export class DraftStoreService {
  async save(draft: DraftState, documents: AttachedDocument[]): Promise<void> {
    const files: StoredDraftBundle['files'] = {};
    for (const document of documents) {
      if (document.file) {
        files[document.id] = {
          name: document.file.name,
          type: document.file.type,
          lastModified: document.file.lastModified,
          file: document.file
        };
      }
    }

    const db = await this.open();
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(STORE, 'readwrite');
      tx.objectStore(STORE).put({ key: KEY, draft, files });
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
    db.close();
  }

  async load(): Promise<StoredDraftBundle | null> {
    const db = await this.open();
    return new Promise<StoredDraftBundle | null>((resolve, reject) => {
      const tx = db.transaction(STORE, 'readonly');
      const req = tx.objectStore(STORE).get(KEY);
      req.onsuccess = () => {
        db.close();
        resolve(req.result?.draft ? { draft: req.result.draft, files: req.result.files ?? {} } : null);
      };
      req.onerror = () => {
        db.close();
        reject(req.error);
      };
    });
  }

  async clear(): Promise<void> {
    const db = await this.open();
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(STORE, 'readwrite');
      tx.objectStore(STORE).delete(KEY);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
    db.close();
  }

  private open(): Promise<IDBDatabase> {
    return new Promise((resolve, reject) => {
      const req = indexedDB.open(DB_NAME, 1);
      req.onupgradeneeded = () => {
        if (!req.result.objectStoreNames.contains(STORE)) req.result.createObjectStore(STORE, { keyPath: 'key' });
      };
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
  }
}
