#!/usr/bin/env python3
"""
Import categories, brands, and products from old project CSVs into the new Tene backend.

Key improvements over old script:
- Products grouped by `code`: color variants become images on one product, not separate products
- Product ID is TEXT (the `code` field from old DB)
- Category mapping uses link_cat with fallback to middle/parent fields
- Brands imported from brands.csv directly

Usage:
    python3 scripts/import_data.py --base-url http://localhost:3000 --token <admin_jwt_token> --db-url <db_url>
"""

import csv
import argparse
import asyncio
import math
import os
import re
from urllib.parse import quote

import aiohttp
import asyncpg

OLD_IMAGE_BASE = "https://ginventor.ge/uploads/images/categories"

NAMED_COLORS = {
    "black": (0, 0, 0),
    "white": (255, 255, 255),
    "red": (255, 0, 0),
    "green": (0, 128, 0),
    "blue": (0, 0, 255),
    "yellow": (255, 255, 0),
    "orange": (255, 165, 0),
    "purple": (128, 0, 128),
    "pink": (255, 192, 203),
    "brown": (139, 69, 19),
    "gray": (128, 128, 128),
    "navy": (0, 0, 128),
    "teal": (0, 128, 128),
    "maroon": (128, 0, 0),
    "olive": (128, 128, 0),
    "cyan": (0, 255, 255),
    "magenta": (255, 0, 255),
    "lime": (0, 255, 0),
    "gold": (255, 215, 0),
    "silver": (192, 192, 192),
    "beige": (245, 245, 220),
    "coral": (255, 127, 80),
    "turquoise": (64, 224, 208),
}


def hex_to_color_name(hex_code):
    hex_code = hex_code.lstrip('#')
    if len(hex_code) != 6:
        return None
    try:
        r, g, b = int(hex_code[0:2], 16), int(hex_code[2:4], 16), int(hex_code[4:6], 16)
    except ValueError:
        return None

    best_name = None
    best_dist = float('inf')
    for name, (nr, ng, nb) in NAMED_COLORS.items():
        dist = math.sqrt((r - nr) ** 2 + (g - ng) ** 2 + (b - nb) ** 2)
        if dist < best_dist:
            best_dist = dist
            best_name = name
    return best_name


def slugify(text):
    text = text.strip().lower()
    text = re.sub(r'[^\w\s-]', '', text)
    text = re.sub(r'[\s_]+', '-', text)
    text = re.sub(r'-+', '-', text)
    return text.strip('-') or 'unnamed'


# ─── BRANDS ───────────────────────────────────────────────────────────────────

async def import_brands(pool, brands_csv_path):
    print("\n=== Importing brands ===")
    brands = {}
    with open(brands_csv_path, 'r', encoding='utf-8') as f:
        reader = csv.DictReader(f)
        for row in reader:
            bid = int(row['id'])
            name = row['title'].strip()
            if name:
                brands[bid] = name

    async with pool.acquire() as conn:
        await conn.executemany(
            "INSERT INTO brands (id, name) VALUES ($1, $2) ON CONFLICT (name) DO NOTHING",
            [(bid, name) for bid, name in sorted(brands.items())]
        )
        await conn.execute("SELECT setval('brands_id_seq', (SELECT COALESCE(MAX(id), 1) FROM brands))")

    print(f"Inserted {len(brands)} brands")
    return brands


# ─── CATEGORIES ───────────────────────────────────────────────────────────────

async def import_categories(pool, categories_csv_path, base_url, token):
    print("\n=== Importing categories ===")
    headers = {
        "Authorization": f"Bearer {token}",
        "Content-Type": "application/json"
    }

    rows = []
    with open(categories_csv_path, 'r', encoding='utf-8') as f:
        reader = csv.DictReader(f)
        for row in reader:
            rows.append(row)

    roots = [r for r in rows if r['parent_id'] == '0']
    children = [r for r in rows if r['parent_id'] != '0']

    old_to_new_id = {}
    created = 0
    failed = 0

    async with aiohttp.ClientSession(
        connector=aiohttp.TCPConnector(ssl=False),
        headers={"User-Agent": "Mozilla/5.0"}
    ) as session:
        for row in roots:
            result = await create_category(session, base_url, headers, row, old_to_new_id)
            if result:
                created += 1
            else:
                failed += 1

        sem = asyncio.Semaphore(10)
        async def limited_create(row):
            async with sem:
                return await create_category(session, base_url, headers, row, old_to_new_id)

        results = await asyncio.gather(*[limited_create(r) for r in children])
        for r in results:
            if r:
                created += 1
            else:
                failed += 1

    print(f"Created {created} categories, {failed} failed")

    # Build link_cat -> new category ID mapping
    link_cat_to_new = {}
    for row in rows:
        lc = row.get('link_cat', '').strip()
        old_id = int(row['id'])
        if lc and lc != '0' and old_id in old_to_new_id:
            link_cat_to_new[lc] = old_to_new_id[old_id]

    return old_to_new_id, link_cat_to_new


async def create_category(session, base_url, headers, row, old_to_new_id):
    old_id = int(row['id'])
    parent_id_old = int(row['parent_id'])
    name = row['title'].strip()
    seo = row.get('seo', '').strip()
    slug = seo if seo else slugify(name)
    description = row.get('seo_bottom_text', '').strip() or None
    display_order = int(row.get('sort', '0'))
    photo = row.get('photo', '').strip()

    parent_id = None
    if parent_id_old != 0:
        parent_id = old_to_new_id.get(parent_id_old)
        if parent_id is None:
            print(f"  SKIP category '{name}' (id={old_id}): parent {parent_id_old} not found")
            return False

    # Ensure unique slug
    slug_base = slug
    slug_counter = 1
    while True:
        payload = {
            "name": name,
            "slug": slug,
            "description": description,
            "display_order": display_order,
            "parent_id": parent_id,
            "enabled": True
        }
        async with session.post(f"{base_url}/admin/categories", headers=headers, json=payload) as resp:
            if resp.status == 200:
                new_cat = await resp.json()
                new_id = new_cat['id']
                old_to_new_id[old_id] = new_id
                if photo:
                    await upload_category_image(session, base_url, headers, new_id, photo)
                return True
            elif resp.status == 409:
                # slug conflict, try with suffix
                slug_counter += 1
                slug = f"{slug_base}-{slug_counter}"
                continue
            else:
                text = await resp.text()
                print(f"  FAIL category '{name}' (id={old_id}): {resp.status} {text}")
                return False


async def upload_category_image(session, base_url, headers, category_id, photo_filename):
    image_url = f"{OLD_IMAGE_BASE}/{photo_filename}"

    ext = photo_filename.rsplit('.', 1)[-1].lower() if '.' in photo_filename else 'jpg'
    content_type_map = {'jpg': 'image/jpeg', 'jpeg': 'image/jpeg', 'png': 'image/png', 'webp': 'image/webp'}
    content_type = content_type_map.get(ext, 'image/jpeg')

    async with session.put(
        f"{base_url}/admin/categories/{category_id}/image",
        headers=headers,
        json={"content_type": content_type}
    ) as resp:
        if resp.status != 200:
            print(f"    FAIL get upload URL for category {category_id}: {resp.status}")
            return
        upload_data = await resp.json()

    presigned_url = upload_data['upload_url']

    try:
        async with session.get(image_url, timeout=aiohttp.ClientTimeout(total=15)) as resp:
            if resp.status != 200:
                print(f"    FAIL fetch image {image_url}: {resp.status}")
                return
            image_data = await resp.read()
    except Exception as e:
        print(f"    FAIL fetch image {image_url}: {e}")
        return

    async with session.put(presigned_url, data=image_data, headers={"Content-Type": content_type}) as resp:
        if resp.status in (200, 204):
            print(f"    Image uploaded for category {category_id}")
        else:
            print(f"    FAIL upload image for category {category_id}: {resp.status}")


# ─── PRODUCTS ─────────────────────────────────────────────────────────────────

def strip_color_suffix(title):
    """Strip trailing color name from product title to find the base product name."""
    color_words = [
        'white', 'black', 'blue', 'red', 'green', 'grey', 'gray', 'gold',
        'silver', 'pink', 'purple', 'yellow', 'orange', 'brown', 'coral',
        'midnight', 'navy', 'teal', 'olive', 'beige', 'turquoise', 'lime',
        'cyan', 'magenta', 'graphite', 'starlight', 'sierra', 'alpine',
        'space gray', 'space grey',
    ]
    lower = title.lower().strip()
    for cw in sorted(color_words, key=len, reverse=True):
        if lower.endswith(cw):
            return title[:len(title) - len(cw)].strip().rstrip(' -/')
    return title


def load_specifications(specs_csv, content_csv, assigned_csv, products_csv):
    """Build mapping: old product_id -> specifications JSON object."""

    # Load spec field definitions
    specs = {}
    with open(specs_csv, 'r', encoding='utf-8') as f:
        for row in csv.DictReader(f):
            specs[row['id']] = row

    # Load spec content values
    contents = {}
    with open(content_csv, 'r', encoding='utf-8') as f:
        for row in csv.DictReader(f):
            contents[row['id']] = row

    # Map old product_id -> code, and code -> category from products CSV
    old_id_to_code = {}
    code_to_category = {}
    with open(products_csv, 'r', encoding='utf-8') as f:
        for row in csv.DictReader(f):
            old_id = row['id']
            code = row.get('code', '').strip()
            cat = row.get('category', '').strip()
            if code:
                old_id_to_code[old_id] = code
                if code not in code_to_category and cat:
                    code_to_category[code] = cat

    # Build per old-product-id grouped specs
    # Filter: only keep specs whose category matches the product's category
    # in the OLD category system. This avoids mixing e.g. laptop specs into
    # a headphone product when the old DB had cross-category assignments.
    product_specs = {}
    with open(assigned_csv, 'r', encoding='utf-8') as f:
        for a in csv.DictReader(f):
            pid = a['product_id']
            spec = specs.get(a['specification_id'])
            content = contents.get(a['specification_content_id'])
            if not spec or not content:
                continue

            # Filter by product's category — but only if we know both
            code = old_id_to_code.get(pid)
            if code:
                product_cat = code_to_category.get(code)
                spec_cat = spec.get('category_id', '')
                if product_cat and spec_cat and spec_cat != product_cat:
                    continue

            val = content['title'].strip()
            if not val or val == '?':
                continue
            name = spec['title'].strip()
            if not name:
                continue

            parent = specs.get(spec.get('parent_id', ''))
            group = parent['title'].strip() if parent and parent.get('title') else 'სხვა'

            if pid not in product_specs:
                product_specs[pid] = {}
            product_specs[pid].setdefault(group, []).append({'name': name, 'value': val})

    # Build set of all known codes for fallback matching
    all_codes = set(old_id_to_code.values())

    # Re-key by code, first match wins
    specs_by_code = {}
    for old_pid, grouped in product_specs.items():
        code = old_id_to_code.get(old_pid)
        if not code:
            # Fallback: try GIN-200{pid} pattern for old IDs not in news.csv
            if old_pid.isdigit():
                candidate = f"GIN-200{old_pid}"
                if candidate in all_codes:
                    code = candidate
            if not code:
                continue
        if code in specs_by_code:
            continue
        specs_by_code[code] = grouped

    print(f"  Loaded specs for {len(specs_by_code)} products (by code)")
    return specs_by_code


async def import_products(pool, products_csv_path, link_cat_to_new, base_url, token, images_dir, specs_by_code, skip_images=False):
    print("\n=== Importing products ===")

    rows = []
    with open(products_csv_path, 'r', encoding='utf-8') as f:
        reader = csv.DictReader(f)
        for row in reader:
            rows.append(row)

    print(f"Total rows in CSV: {len(rows)}")

    # ── Group rows by product code ──
    # Products with the same `code` are color variants of the same product.
    # For products with different codes but identical base names (after stripping
    # color suffix), we still treat them as separate products — they have different
    # barcodes/SKUs.
    products = {}  # code -> {product_data, images: [{photo, color, row}]}

    for row in rows:
        code = row.get('code', '').strip()
        if not code:
            continue

        name = row.get('title', '').strip()
        if not name:
            continue

        photo = row.get('photo', '').strip()
        color_hex = row.get('color', '').strip()
        color_name = hex_to_color_name(color_hex) if color_hex else None
        stock = int(row.get('stock', '0').strip() or '0')

        if code not in products:
            # First row for this code — this becomes the product
            description = row.get('text', '').strip() or None
            price_str = row.get('price', '').strip()
            try:
                price = float(price_str) if price_str else 0.0
            except ValueError:
                cleaned = re.sub(r'\.{2,}', '.', price_str)
                try:
                    price = float(cleaned)
                except ValueError:
                    price = 0.0

            sale_percent = float(row.get('sale_percent', '0').strip() or '0')
            brand_raw = row.get('brand', '').strip()
            brand_id = int(brand_raw) if brand_raw and brand_raw != '0' else None

            warranty = parse_warranty(
                row.get('guarantee_amount', ''),
                row.get('guarantee_type', '')
            )
            active = row.get('active', '0').strip()
            enabled = active == '1'

            # Resolve category via link_cat mapping: try category, then middle, then parent
            cat_val = row.get('category', '').strip()
            mid_val = row.get('middle', '').strip()
            par_val = row.get('parent', '').strip()
            new_cat_id = None
            for val in [cat_val, mid_val, par_val]:
                if val in link_cat_to_new:
                    new_cat_id = link_cat_to_new[val]
                    break

            # Use the base name (stripped of color suffix) as product name
            base_name = strip_color_suffix(name)

            products[code] = {
                'id': code,
                'name': base_name,
                'description': description,
                'price': price,
                'discount': sale_percent,
                'brand_id': brand_id,
                'warranty': warranty,
                'enabled': enabled,
                'category_id': new_cat_id,
                'specifications': specs_by_code.get(code, {}),
                'images': [],
            }

        # Add image for this variant
        if photo:
            products[code]['images'].append({
                'photo': photo,
                'color': color_name,
                'quantity': stock,
            })

    print(f"Unique products (by code): {len(products)}")
    print(f"Total images: {sum(len(p['images']) for p in products.values())}")

    # ── Bulk insert products ──
    import json

    product_records = []
    category_records = []
    specs_count = 0

    for code, prod in products.items():
        spec_json = json.dumps(prod['specifications'], ensure_ascii=False) if prod['specifications'] else '{}'
        if prod['specifications']:
            specs_count += 1
        product_records.append((
            prod['id'], prod['name'], prod['description'],
            prod['price'], prod['discount'], 0,
            spec_json,
            prod['brand_id'], prod['warranty'], prod['enabled']
        ))
        if prod['category_id']:
            category_records.append((prod['id'], prod['category_id']))

    inserted = 0
    skipped = 0
    BATCH_SIZE = 500

    async with pool.acquire() as conn:
        for i in range(0, len(product_records), BATCH_SIZE):
            batch = product_records[i:i + BATCH_SIZE]
            try:
                await conn.executemany(
                    """INSERT INTO products (id, name, description, price, discount, quantity, specifications, brand_id, warranty, enabled)
                       VALUES ($1, $2, $3, $4, $5, $6, $7::jsonb, $8, $9, $10)
                       ON CONFLICT (id) DO UPDATE SET
                           name = EXCLUDED.name,
                           description = COALESCE(EXCLUDED.description, products.description),
                           price = EXCLUDED.price,
                           discount = EXCLUDED.discount,
                           specifications = EXCLUDED.specifications,
                           brand_id = COALESCE(EXCLUDED.brand_id, products.brand_id),
                           warranty = COALESCE(EXCLUDED.warranty, products.warranty),
                           enabled = EXCLUDED.enabled""",
                    batch
                )
                inserted += len(batch)
                print(f"  Inserted batch {i // BATCH_SIZE + 1} ({inserted}/{len(product_records)})")
            except Exception as e:
                print(f"  FAIL batch {i // BATCH_SIZE + 1}: {e}")
                skipped += len(batch)

        # Bulk insert category mappings
        if category_records:
            try:
                await conn.executemany(
                    """INSERT INTO product_categories (product_id, category_id)
                       VALUES ($1, $2) ON CONFLICT DO NOTHING""",
                    category_records
                )
                print(f"  Assigned {len(category_records)} product-category mappings")
            except Exception as e:
                print(f"  FAIL category assignments: {e}")

    print(f"Inserted {inserted} products, skipped {skipped}")
    print(f"Products with category: {len(category_records)}/{len(product_records)}")
    print(f"Products with specifications: {specs_count}")

    if skip_images:
        return

    # ── Upload product images ──
    print("\n=== Uploading product images ===")
    headers = {
        "Authorization": f"Bearer {token}",
        "Content-Type": "application/json"
    }
    sem = asyncio.Semaphore(20)

    async with aiohttp.ClientSession(
        connector=aiohttp.TCPConnector(ssl=False),
        headers={"User-Agent": "Mozilla/5.0"}
    ) as session:
        tasks = []
        for code, prod in products.items():
            for idx, img in enumerate(prod['images']):
                is_primary = (idx == 0)
                tasks.append((
                    session, sem, base_url, headers, code,
                    img['photo'], images_dir, img['color'], is_primary,
                    img['quantity']
                ))

        progress = {'done': 0, 'total': len(tasks)}
        print(f"Total images to upload: {progress['total']}")
        coros = [upload_product_image(*t, progress) for t in tasks]

        results = await asyncio.gather(*coros, return_exceptions=True)
        images_uploaded = sum(1 for r in results if r is True)

    print(f"Uploaded {images_uploaded} product images")


def parse_warranty(amount, gtype):
    amount = amount.strip()
    gtype = gtype.strip()
    if not amount or amount == '0' or gtype == '0':
        return None
    unit = 'year' if gtype == '1' else 'month'
    suffix = 's' if amount != '1' else ''
    return f"{amount} {unit}{suffix}"


async def upload_product_image(session, sem, base_url, headers, product_id, photo_filename, images_dir, color, is_primary, quantity, progress):
    async with sem:
        ext = photo_filename.rsplit('.', 1)[-1].lower() if '.' in photo_filename else 'jpg'
        content_type_map = {'jpg': 'image/jpeg', 'jpeg': 'image/jpeg', 'png': 'image/png', 'webp': 'image/webp'}
        content_type = content_type_map.get(ext, 'image/jpeg')

        image_path = os.path.join(images_dir, photo_filename)
        if not os.path.exists(image_path):
            progress['done'] += 1
            return False

        with open(image_path, 'rb') as f:
            image_data = f.read()

        payload = {"images": [{"content_type": content_type, "is_primary": is_primary, "color": color, "quantity": quantity}]}
        async with session.put(
            f"{base_url}/admin/products/{quote(product_id, safe='')}/images",
            headers=headers,
            json=payload
        ) as resp:
            if resp.status != 200:
                progress['done'] += 1
                text = await resp.text()
                print(f"    FAIL get upload URL for product {product_id}: {resp.status} {text}")
                return False
            data = await resp.json()

        if not data.get('images'):
            progress['done'] += 1
            return False

        presigned_url = data['images'][0]['upload_url']

        async with session.put(presigned_url, data=image_data, headers={"Content-Type": content_type}) as resp:
            progress['done'] += 1
            if resp.status in (200, 204):
                if progress['done'] % 100 == 0 or progress['done'] == progress['total']:
                    print(f"    [{progress['done']}/{progress['total']}] Progress...")
                return True
            else:
                print(f"    [{progress['done']}/{progress['total']}] FAIL upload for product {product_id}: {resp.status}")
                return False


# ─── MAIN ─────────────────────────────────────────────────────────────────────

async def main():
    parser = argparse.ArgumentParser(description="Import data from old project CSVs")
    parser.add_argument("--base-url", default="http://localhost:3000", help="Backend API base URL")
    parser.add_argument("--token", required=True, help="Admin JWT token")
    parser.add_argument("--db-url", required=True, help="Database URL")
    parser.add_argument("--categories-csv", default="tests/categories_new.csv")
    parser.add_argument("--products-csv", default="tests/news.csv")
    parser.add_argument("--brands-csv", default="tests/brands.csv")
    parser.add_argument("--specs-csv", default="tests/specifications.csv")
    parser.add_argument("--spec-content-csv", default="tests/specification_content.csv")
    parser.add_argument("--spec-assigned-csv", default="tests/specification_content_assigned.csv")
    parser.add_argument("--skip-categories", action="store_true")
    parser.add_argument("--skip-brands", action="store_true")
    parser.add_argument("--skip-products", action="store_true")
    parser.add_argument("--images-dir", default="tests/product_images_full")
    parser.add_argument("--skip-images", action="store_true")
    args = parser.parse_args()

    pool = await asyncpg.create_pool(args.db_url)

    try:
        if not args.skip_brands:
            await import_brands(pool, args.brands_csv)

        link_cat_to_new = {}
        if not args.skip_categories:
            _, link_cat_to_new = await import_categories(
                pool, args.categories_csv, args.base_url, args.token
            )
            print(f"\nlink_cat -> new category ID mapping ({len(link_cat_to_new)} entries)")

        if not args.skip_products:
            if not link_cat_to_new:
                print("Note: no category mapping available, products won't be assigned categories")

            specs_by_code = load_specifications(
                args.specs_csv, args.spec_content_csv,
                args.spec_assigned_csv, args.products_csv
            )

            await import_products(
                pool, args.products_csv, link_cat_to_new,
                args.base_url, args.token, args.images_dir, specs_by_code, args.skip_images
            )

        print("\nDone!")
    finally:
        await pool.close()


if __name__ == "__main__":
    asyncio.run(main())
