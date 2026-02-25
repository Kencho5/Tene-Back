#!/usr/bin/env python3
"""
Import categories, brands, and products from old project CSVs into the new Tene backend.

Usage:
    python3 scripts/import_data.py --base-url http://localhost:3000 --token <admin_jwt_token>

Requires:
    pip install aiohttp asyncpg
"""

import csv
import argparse
import asyncio
import math
import os
import re

import aiohttp
import asyncpg

OLD_IMAGE_BASE = "https://ginventor.ge/uploads/images/categories"
OLD_PRODUCT_IMAGE_BASE = "https://ginventor.ge/uploads/photos/news"


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
    """Convert hex color code to nearest named color."""
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


async def import_brands(pool, products_csv_path):
    print("\n=== Importing brands ===")
    brands = {}
    with open(products_csv_path, 'r', encoding='utf-8') as f:
        reader = csv.DictReader(f)
        for row in reader:
            bid = row.get('brand', '').strip()
            btitle = row.get('brand_title', '').strip()
            if bid and btitle and bid != '0':
                brands[int(bid)] = btitle

    async with pool.acquire() as conn:
        for old_id, name in sorted(brands.items()):
            await conn.execute(
                "INSERT INTO brands (id, name) VALUES ($1, $2) ON CONFLICT (name) DO NOTHING",
                old_id, name
            )
        await conn.execute("SELECT setval('brands_id_seq', (SELECT COALESCE(MAX(id), 1) FROM brands))")

    print(f"Inserted {len(brands)} brands")
    return brands


async def import_categories(categories_csv_path, base_url, token):
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
        headers={"User-Agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36"}
    ) as session:
        # Import roots first (must be sequential for ID mapping)
        for row in roots:
            result = await create_category(session, base_url, headers, row, old_to_new_id)
            if result:
                created += 1
            else:
                failed += 1

        # Import children concurrently in batches
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
    return old_to_new_id


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

    payload = {
        "name": name,
        "slug": slug,
        "description": description,
        "display_order": display_order,
        "parent_id": parent_id,
        "enabled": True
    }

    async with session.post(f"{base_url}/admin/categories", headers=headers, json=payload) as resp:
        if resp.status != 200:
            text = await resp.text()
            print(f"  FAIL category '{name}' (id={old_id}): {resp.status} {text}")
            return False
        new_cat = await resp.json()

    new_id = new_cat['id']
    old_to_new_id[old_id] = new_id

    if photo:
        await upload_category_image(session, base_url, headers, new_id, photo)

    return True


async def upload_category_image(session, base_url, headers, category_id, photo_filename):
    image_url = f"{OLD_IMAGE_BASE}/{photo_filename}"

    ext = photo_filename.rsplit('.', 1)[-1].lower() if '.' in photo_filename else 'jpg'
    content_type_map = {'jpg': 'image/jpeg', 'jpeg': 'image/jpeg', 'png': 'image/png', 'webp': 'image/webp'}
    content_type = content_type_map.get(ext, 'image/jpeg')

    # Get presigned upload URL
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

    # Fetch old image
    try:
        async with session.get(image_url, timeout=aiohttp.ClientTimeout(total=15)) as resp:
            if resp.status != 200:
                print(f"    FAIL fetch image {image_url}: {resp.status}")
                return
            image_data = await resp.read()
    except Exception as e:
        print(f"    FAIL fetch image {image_url}: {e}")
        return

    # Upload to S3
    async with session.put(presigned_url, data=image_data, headers={"Content-Type": content_type}) as resp:
        if resp.status in (200, 204):
            print(f"    Image uploaded for category {category_id}")
        else:
            print(f"    FAIL upload image for category {category_id}: {resp.status}")


INT32_MAX = 2_147_483_647

def parse_product_id(code, fallback_id):
    """Extract numeric ID from code like ALT-117371 -> 117371. Falls back to old id if out of int32 range."""
    nums = re.sub(r'[^0-9]', '', code)
    if nums:
        val = int(nums)
        if val <= INT32_MAX:
            return val
    # Fallback to old numeric id
    fallback = fallback_id.strip()
    if fallback:
        return int(fallback)
    return None


def parse_warranty(amount, gtype):
    """Convert guarantee_amount + guarantee_type to warranty string."""
    amount = amount.strip()
    gtype = gtype.strip()
    if not amount or amount == '0' or gtype == '0':
        return None
    unit = 'year' if gtype == '1' else 'month'
    suffix = 's' if amount != '1' else ''
    return f"{amount} {unit}{suffix}"


def build_link_cat_map(categories_csv_path):
    """Build mapping: link_cat value -> old category id."""
    link_cat_to_old = {}
    with open(categories_csv_path, 'r', encoding='utf-8') as f:
        reader = csv.DictReader(f)
        for row in reader:
            lc = row.get('link_cat', '').strip()
            if lc and lc != '0':
                link_cat_to_old[lc] = int(row['id'])
    return link_cat_to_old


async def import_products(pool, products_csv_path, categories_csv_path, cat_old_to_new, base_url, token, images_dir, skip_images=False):
    print("\n=== Importing products ===")

    link_cat_map = build_link_cat_map(categories_csv_path)

    rows = []
    with open(products_csv_path, 'r', encoding='utf-8') as f:
        reader = csv.DictReader(f)
        for row in reader:
            rows.append(row)

    print(f"Total rows in CSV: {len(rows)}")

    inserted = 0
    skipped = 0
    images_uploaded = 0

    # Insert products into DB
    async with pool.acquire() as conn:
        for row in rows:
            code = row.get('code', '').strip()
            product_id = parse_product_id(code, row.get('id', ''))
            if product_id is None:
                skipped += 1
                continue

            name = row.get('title', '').strip()
            if not name:
                skipped += 1
                continue

            description = row.get('text', '').strip() or None
            price_str = row.get('price', '').strip()
            try:
                price = float(price_str) if price_str else 0.0
            except ValueError:
                # Fix malformed prices like "289..99"
                cleaned = re.sub(r'\.{2,}', '.', price_str)
                try:
                    price = float(cleaned)
                except ValueError:
                    price = 0.0
            sale_percent = float(row.get('sale_percent', '0').strip() or '0')
            stock = int(row.get('stock', '0').strip() or '0')
            brand_raw = row.get('brand', '').strip()
            brand_id = int(brand_raw) if brand_raw and brand_raw != '0' else None
            warranty = parse_warranty(
                row.get('guarantee_amount', ''),
                row.get('guarantee_type', '')
            )
            active = row.get('active', '0').strip()
            enabled = active == '1'

            try:
                await conn.execute(
                    """INSERT INTO products (id, name, description, price, discount, quantity, specifications, brand_id, warranty, enabled)
                       VALUES ($1, $2, $3, $4, $5, $6, '{}'::jsonb, $7, $8, $9)
                       ON CONFLICT (id) DO UPDATE SET
                           brand_id = COALESCE(EXCLUDED.brand_id, products.brand_id),
                           warranty = COALESCE(EXCLUDED.warranty, products.warranty),
                           enabled = EXCLUDED.enabled""",
                    product_id, name, description, price, sale_percent, stock,
                    brand_id, warranty, enabled
                )
                inserted += 1
            except Exception as e:
                print(f"  FAIL product {product_id} '{name}': {e}")
                skipped += 1
                continue

            # Map product to category
            cat_val = row.get('category', '').strip()
            mid_val = row.get('middle', '').strip()
            par_val = row.get('parent', '').strip()

            # Try category -> middle -> parent via link_cat mapping
            old_cat_id = None
            for val in [cat_val, mid_val, par_val]:
                if val in link_cat_map:
                    old_cat_id = link_cat_map[val]
                    break

            if old_cat_id and old_cat_id in cat_old_to_new:
                new_cat_id = cat_old_to_new[old_cat_id]
                try:
                    await conn.execute(
                        """INSERT INTO product_categories (product_id, category_id)
                           VALUES ($1, $2) ON CONFLICT DO NOTHING""",
                        product_id, new_cat_id
                    )
                except Exception as e:
                    print(f"  FAIL category assign for product {product_id}: {e}")

    print(f"Inserted {inserted} products, skipped {skipped}")

    if skip_images:
        return

    # Upload product images concurrently
    print("\n=== Uploading product images ===")
    headers = {
        "Authorization": f"Bearer {token}",
        "Content-Type": "application/json"
    }
    sem = asyncio.Semaphore(20)

    async with aiohttp.ClientSession(
        connector=aiohttp.TCPConnector(ssl=False),
        headers={"User-Agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36"}
    ) as session:
        tasks = []
        for row in rows:
            code = row.get('code', '').strip()
            product_id = parse_product_id(code, row.get('id', ''))
            photo = row.get('photo', '').strip()
            color = hex_to_color_name(row.get('color', '').strip())
            if product_id and photo:
                tasks.append((session, sem, base_url, headers, product_id, photo, images_dir, color))

        progress = {'done': 0, 'total': len(tasks)}
        print(f"Total images to upload: {progress['total']}")
        tasks = [upload_product_image(*t, progress) for t in tasks]

        results = await asyncio.gather(*tasks, return_exceptions=True)
        images_uploaded = sum(1 for r in results if r is True)

    print(f"Uploaded {images_uploaded} product images")


async def upload_product_image(session, sem, base_url, headers, product_id, photo_filename, images_dir, color, progress):
    async with sem:
        ext = photo_filename.rsplit('.', 1)[-1].lower() if '.' in photo_filename else 'jpg'
        content_type_map = {'jpg': 'image/jpeg', 'jpeg': 'image/jpeg', 'png': 'image/png', 'webp': 'image/webp'}
        content_type = content_type_map.get(ext, 'image/jpeg')

        # Read image from local directory
        image_path = os.path.join(images_dir, photo_filename)
        if not os.path.exists(image_path):
            print(f"    SKIP product {product_id}: image not found {image_path}")
            return False

        with open(image_path, 'rb') as f:
            image_data = f.read()

        # Get presigned URL via admin API
        payload = {"images": [{"content_type": content_type, "is_primary": True, "color": color}]}
        async with session.put(
            f"{base_url}/admin/products/{product_id}/images",
            headers=headers,
            json=payload
        ) as resp:
            if resp.status != 200:
                print(f"    FAIL get upload URL for product {product_id}: {resp.status}")
                return False
            data = await resp.json()

        if not data.get('images'):
            print(f"    FAIL no upload URL returned for product {product_id}")
            return False

        presigned_url = data['images'][0]['upload_url']

        # Upload to S3
        async with session.put(presigned_url, data=image_data, headers={"Content-Type": content_type}) as resp:
            progress['done'] += 1
            if resp.status in (200, 204):
                print(f"    [{progress['done']}/{progress['total']}] Uploaded image for product {product_id}")
                return True
            else:
                print(f"    [{progress['done']}/{progress['total']}] FAIL upload image for product {product_id}: {resp.status}")
                return False


async def main():
    parser = argparse.ArgumentParser(description="Import data from old project CSVs")
    parser.add_argument("--base-url", default="http://localhost:3000", help="Backend API base URL")
    parser.add_argument("--token", required=True, help="Admin JWT token")
    parser.add_argument("--db-url", required=True, help="Database URL")
    parser.add_argument("--categories-csv", default="tests/categories_new.csv", help="Categories CSV path")
    parser.add_argument("--products-csv", default="tests/news.csv", help="Products CSV path")
    parser.add_argument("--skip-categories", action="store_true", help="Skip category import")
    parser.add_argument("--skip-brands", action="store_true", help="Skip brand import")
    parser.add_argument("--skip-products", action="store_true", help="Skip product import")
    parser.add_argument("--images-dir", default="tests/product_images_full", help="Local directory with product images")
    parser.add_argument("--skip-images", action="store_true", help="Skip product image upload")
    args = parser.parse_args()

    pool = await asyncpg.create_pool(args.db_url)

    try:
        if not args.skip_brands:
            await import_brands(pool, args.products_csv)

        cat_old_to_new = {}
        if not args.skip_categories:
            cat_old_to_new = await import_categories(args.categories_csv, args.base_url, args.token)
            print(f"\nCategory ID mapping (old -> new): {cat_old_to_new}")

        if not args.skip_products:
            # If categories were skipped, try to rebuild mapping from DB
            if not cat_old_to_new:
                print("Note: no category mapping available, products won't be assigned categories")
            await import_products(pool, args.products_csv, args.categories_csv, cat_old_to_new, args.base_url, args.token, args.images_dir, args.skip_images)

        print("\nDone!")
    finally:
        await pool.close()


if __name__ == "__main__":
    asyncio.run(main())
