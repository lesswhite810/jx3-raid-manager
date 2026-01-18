import { RaidRecord } from '../types';

export interface MergeResult {
  mergedRecords: RaidRecord[];
  deletedRecords: string[];
  updatedRecords: string[];
}

export const generateTransactionId = (): string => {
  return `txn_${Date.now()}_${Math.random().toString(36).substring(2, 9)}`;
};

export const findDuplicateRecords = (records: RaidRecord[]): Map<string, RaidRecord[]> => {
  const groups = new Map<string, RaidRecord[]>();
  
  records.forEach(record => {
    const key = `${record.raidName}_${record.roleId}_${record.date}`;
    if (!groups.has(key)) {
      groups.set(key, []);
    }
    groups.get(key)!.push(record);
  });
  
  const duplicates = new Map<string, RaidRecord[]>();
  groups.forEach((group, key) => {
    if (group.length > 1) {
      duplicates.set(key, group);
    }
  });
  
  return duplicates;
};

export const mergeRecords = (records: RaidRecord[]): MergeResult => {
  const duplicates = findDuplicateRecords(records);
  const keptRecords: RaidRecord[] = [];
  const deletedIds: string[] = [];
  const updatedIds: string[] = [];
  
  const processedPairs = new Set<string>();
  
  duplicates.forEach((group) => {
    const incomeRecord = group.find(r => r.transactionType === 'income' || r.goldIncome > 0);
    const expenseRecord = group.find(r => r.transactionType === 'expense' || (r.goldExpense && r.goldExpense > 0));
    
    if (incomeRecord && expenseRecord && incomeRecord.id !== expenseRecord.id) {
      const pairKey = [incomeRecord.id, expenseRecord.id].sort().join('_');
      if (!processedPairs.has(pairKey)) {
        processedPairs.add(pairKey);
        
        const transactionId = generateTransactionId();
        
        const mergedRecord: RaidRecord = {
          ...incomeRecord,
          id: incomeRecord.id,
          transactionId: transactionId,
          goldIncome: incomeRecord.goldIncome,
          goldExpense: expenseRecord.goldExpense || 0,
          hasXuanjing: incomeRecord.hasXuanjing || expenseRecord.hasXuanjing,
          notes: mergeNotes(incomeRecord.notes, expenseRecord.notes),
          transactionType: 'combined',
          isCleared: (incomeRecord.isCleared && expenseRecord.isCleared) || false
        };
        
        keptRecords.push(mergedRecord);
        deletedIds.push(expenseRecord.id);
        updatedIds.push(incomeRecord.id);
      }
    } else if (group.length > 1) {
      const pairKey = group.map(r => r.id).sort().join('_');
      if (!processedPairs.has(pairKey)) {
        processedPairs.add(pairKey);
        
        const primary = group[0];
        const mergedRecord: RaidRecord = {
          ...primary,
          transactionId: generateTransactionId(),
          goldIncome: Math.max(...group.map(r => r.goldIncome)),
          goldExpense: group.reduce((sum, r) => sum + (r.goldExpense || 0), 0),
          hasXuanjing: group.some(r => r.hasXuanjing),
          transactionType: 'combined'
        };
        
        keptRecords.push(mergedRecord);
        for (let i = 1; i < group.length; i++) {
          deletedIds.push(group[i].id);
        }
        updatedIds.push(primary.id);
      }
    } else {
      keptRecords.push(group[0]);
    }
  });
  
  const processedIds = new Set<string>();
  duplicates.forEach(group => {
    group.forEach(r => processedIds.add(r.id));
  });
  
  records.forEach(record => {
    if (!processedIds.has(record.id)) {
      keptRecords.push(record);
    }
  });
  
  return {
    mergedRecords: keptRecords,
    deletedRecords: deletedIds,
    updatedRecords: updatedIds
  };
};

const mergeNotes = (notes1?: string, notes2?: string): string | undefined => {
  const parts: string[] = [];
  if (notes1) parts.push(notes1);
  if (notes2 && notes2 !== notes1) parts.push(notes2);
  return parts.length > 0 ? parts.join('; ') : undefined;
};

export const deduplicateRecords = (records: RaidRecord[]): RaidRecord[] => {
  const seen = new Map<string, RaidRecord>();
  const result: RaidRecord[] = [];
  
  records.forEach(record => {
    const key = `${record.raidName}_${record.roleId}_${record.date}`;
    
    if (!seen.has(key)) {
      seen.set(key, record);
      result.push(record);
    } else {
      const existing = seen.get(key)!;
      
      const combined: RaidRecord = {
        ...existing,
        goldIncome: Math.max(existing.goldIncome, record.goldIncome),
        goldExpense: Math.max(existing.goldExpense || 0, record.goldExpense || 0),
        hasXuanjing: existing.hasXuanjing || record.hasXuanjing,
        isCleared: (existing.isCleared && record.isCleared) || false,
        transactionType: 'combined'
      };
      
      seen.set(key, combined);
      const index = result.findIndex(r => r.id === existing.id);
      if (index !== -1) {
        result[index] = combined;
      }
    }
  });
  
  return result;
};

export const calculateRecordTotal = (record: RaidRecord): number => {
  const income = record.goldIncome || 0;
  const expense = record.goldExpense || 0;
  return income - expense;
};

export const formatGoldAmount = (amount: number): string => {
  if (Math.abs(amount) >= 10000) {
    return `${(amount / 10000).toFixed(2)}w`;
  }
  return amount.toLocaleString();
};
