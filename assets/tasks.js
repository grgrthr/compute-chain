// ═══ COMPUTE CHAIN — REAL TASKS ENGINE ═══

const TaskEngine = {
    // ═══ IMAGE COMPRESSION ═══
    async compressImage(imageData, quality = 0.5) {
        return new Promise((resolve) => {
            const img = new Image();
            img.onload = () => {
                const canvas = document.createElement('canvas');
                canvas.width = img.width;
                canvas.height = img.height;
                const ctx = canvas.getContext('2d');
                ctx.drawImage(img, 0, 0);
                canvas.toBlob((blob) => {
                    resolve({
                        originalSize: imageData.byteLength,
                        compressedSize: blob.size,
                        compressionRatio: (blob.size / imageData.byteLength * 100).toFixed(1) + '%',
                        type: 'image_compression'
                    });
                }, 'image/jpeg', quality);
            };
            img.src = URL.createObjectURL(new Blob([imageData]));
        });
    },

    // ═══ IMAGE THUMBNAIL ═══
    async generateThumbnail(imageData, maxWidth = 200, maxHeight = 200) {
        return new Promise((resolve) => {
            const img = new Image();
            img.onload = () => {
                const canvas = document.createElement('canvas');
                let w = img.width, h = img.height;
                if (w > h) { h = h * maxWidth / w; w = maxWidth; }
                else { w = w * maxHeight / h; h = maxHeight; }
                canvas.width = w; canvas.height = h;
                const ctx = canvas.getContext('2d');
                ctx.drawImage(img, 0, 0, w, h);
                canvas.toBlob((blob) => {
                    resolve({
                        originalSize: imageData.byteLength,
                        thumbnailSize: blob.size,
                        dimensions: `${Math.round(w)}x${Math.round(h)}`,
                        type: 'thumbnail'
                    });
                }, 'image/jpeg', 0.7);
            };
            img.src = URL.createObjectURL(new Blob([imageData]));
        });
    },

    // ═══ CSV STATISTICS ═══
    analyzeCSV(csvText) {
        const lines = csvText.trim().split('\n');
        const headers = lines[0].split(',');
        const data = lines.slice(1).map(line => line.split(',').map(Number));
        
        const stats = {};
        headers.forEach((header, col) => {
            const values = data.map(row => row[col]).filter(v => !isNaN(v));
            if (values.length === 0) return;
            values.sort((a, b) => a - b);
            const sum = values.reduce((a, b) => a + b, 0);
            const mean = sum / values.length;
            const median = values[Math.floor(values.length / 2)];
            const min = values[0];
            const max = values[values.length - 1];
            stats[header.trim()] = { mean: mean.toFixed(2), median, min, max, count: values.length };
        });
        
        return { type: 'csv_analysis', stats, rows: data.length, columns: headers.length };
    },

    // ═══ FILE HASHING (SHA-256) ═══
    async hashFile(fileData) {
        const buffer = await crypto.subtle.digest('SHA-256', fileData);
        const hash = Array.from(new Uint8Array(buffer))
            .map(b => b.toString(16).padStart(2, '0')).join('');
        return {
            type: 'file_hash',
            hash: hash,
            sizeBytes: fileData.byteLength,
            algorithm: 'SHA-256'
        };
    }
};

// ═══ TASK DISPATCHER ═══
async function executeRealTask(taskType, taskData) {
    let result;
    const startTime = performance.now();
    
    switch(taskType) {
        case 'compress':
            result = await TaskEngine.compressImage(taskData, 0.5);
            break;
        case 'thumbnail':
            result = await TaskEngine.generateThumbnail(taskData, 200, 200);
            break;
        case 'csv':
            const text = new TextDecoder().decode(taskData);
            result = TaskEngine.analyzeCSV(text);
            break;
        case 'hash':
            result = await TaskEngine.hashFile(taskData);
            break;
        default:
            result = { type: 'unknown', error: 'Unknown task type' };
    }
    
    result.executionTimeMs = Math.round(performance.now() - startTime);
    return result;
}
